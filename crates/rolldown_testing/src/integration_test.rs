use std::path::PathBuf;
use std::sync::Arc;
use std::{
  fs,
  io::{Read, Write},
  path::Path,
  process::Command,
};

use anyhow::Context;
use rolldown::dev::DevOptions;
use rolldown::dev::dev_options::DevWatchOptions;
use rolldown::{
  BundleOutput, Bundler, BundlerOptions, IsExternal, OutputFormat, Platform, SourceMapType,
  plugin::__inner::SharedPluginable,
};
use rolldown::{DevEngine, NormalizedBundlerOptions};
use rolldown_error::BuildResult;
use rolldown_testing_config::TestMeta;
use serde_json::{Map, Value};
use sugar_path::SugarPath;
use tokio::sync::Mutex;

use crate::hmr_files::{
  apply_hmr_edit_files_to_hmr_temp_dir, collect_hmr_edit_files,
  copy_non_hmr_edit_files_to_hmr_temp_dir, get_changed_files_from_hmr_edit_files,
};
use crate::types::{ArtifactsSnapshot, BuildRoundOutput};

#[derive(Default)]
pub struct IntegrationTest {
  test_meta: TestMeta,
  // Absolute path of the test folder. It may or may not contain the `_config.json` file.
  test_folder_path: PathBuf,
}

pub struct NamedBundlerOptions {
  /// To show the purpose of this config. Will be `None` for the base config.
  pub description: Option<String>,
  pub options: BundlerOptions,
  // Whether to include the output in the snapshot for this config variant. If not specified, `TestMeta.snapshot` will be used.
  pub snapshot: Option<bool>,
  // Will be injected into `globalThis.__configName`. If not specified, `TestMeta.config_name` will be used.
  pub config_name: Option<String>,
}

fn default_test_input_item() -> rolldown::InputItem {
  rolldown::InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }
}

impl IntegrationTest {
  pub fn new(test_meta: TestMeta, test_folder_path: PathBuf) -> Self {
    Self { test_meta, test_folder_path }
  }

  pub async fn bundle(&self, mut options: BundlerOptions) -> BuildResult<BundleOutput> {
    self.apply_test_defaults(&mut options);

    let mut bundler = Bundler::new(options)?;

    if self.test_meta.write_to_disk {
      if bundler.options().out_dir.as_path().is_dir() {
        std::fs::remove_dir_all(&bundler.options().out_dir)
          .context(bundler.options().out_dir.clone())
          .expect("Failed to clean the output directory");
      }
      bundler.write().await
    } else {
      bundler.generate().await
    }
  }

  pub async fn run(&self, options: BundlerOptions) {
    self.run_with_plugins(options, vec![]).await;
  }

  pub async fn run_with_plugins(&self, options: BundlerOptions, plugins: Vec<SharedPluginable>) {
    self
      .run_multiple(
        vec![NamedBundlerOptions { options, description: None, snapshot: None, config_name: None }],
        plugins,
      )
      .await;
  }

  #[expect(clippy::unnecessary_debug_formatting)]
  pub async fn run_multiple(
    &self,
    multiple_options: Vec<NamedBundlerOptions>,
    plugins: Vec<SharedPluginable>,
  ) {
    // Example: crates/rolldown/tests/rolldown/topics/hmr/runtime_correctness
    let test_folder_path = &self.test_folder_path;

    let hmr_temp_dir_path = test_folder_path.join("hmr-temp");
    let hmr_steps = collect_hmr_edit_files(test_folder_path, &hmr_temp_dir_path);
    let hmr_mode_enabled = !hmr_steps.is_empty();

    let mut artifacts_snapshot = ArtifactsSnapshot::default();

    for mut named_options in multiple_options {
      let mut build_snapshot = BuildRoundOutput {
        overwritten_test_meta_snapshot: named_options.snapshot.unwrap_or(self.test_meta.snapshot),
        ..Default::default()
      };
      self.apply_test_defaults(&mut named_options.options);

      if hmr_mode_enabled {
        fs::remove_dir_all(&hmr_temp_dir_path)
          .or_else(|err| if err.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(err) })
          .unwrap();
        copy_non_hmr_edit_files_to_hmr_temp_dir(test_folder_path, &hmr_temp_dir_path);

        named_options.options.cwd = Some(hmr_temp_dir_path.clone());
      }

      let output_dir = format!(
        "{}/{}",
        named_options.options.cwd.as_ref().map_or(".", |cwd| cwd.to_str().unwrap()),
        named_options.options.dir.as_ref().map_or("dist", |v| v)
      );

      let debug_title = named_options.description.clone().unwrap_or_else(String::new);
      if !debug_title.is_empty() {
        build_snapshot.debug_title = Some(debug_title.clone());
      }

      if hmr_mode_enabled {
        let options = named_options.options.clone();
        // FIXME: hyf0 we shouln't use the same bundler for both hmr and non-hmr mode.
        let bundler =
          Bundler::with_plugins(options, plugins.clone()).expect("Failed to create bundler");
        let cwd = bundler.options().cwd.clone();
        build_snapshot.cwd = Some(cwd.clone());
        let bundler = Arc::new(Mutex::new(bundler));

        let hmr_update_infos = Arc::new(std::sync::Mutex::new(vec![]));
        let build_results = Arc::new(std::sync::Mutex::new(vec![]));
        let dev_engine = DevEngine::with_bundler(
          Arc::clone(&bundler),
          DevOptions {
            on_hmr_updates: {
              let hmr_update_infos = Arc::clone(&hmr_update_infos);
              Some(Arc::new(move |result| {
                hmr_update_infos.lock().unwrap().push(result);
              }))
            },
            on_output: {
              let build_results = Arc::clone(&build_results);
              Some(Arc::new(move |bundle_result| {
                build_results.lock().unwrap().push(bundle_result);
              }))
            },
            watch: Some(DevWatchOptions {
              disable_watcher: Some(true),
              skip_write: Some(!self.test_meta.write_to_disk),
              ..Default::default()
            }),
            ..Default::default()
          },
        )
        .unwrap();

        dev_engine.run().await.unwrap();
        dev_engine.create_client_for_testing();

        for hmr_edit_files in &hmr_steps {
          apply_hmr_edit_files_to_hmr_temp_dir(
            test_folder_path,
            &hmr_temp_dir_path,
            hmr_edit_files,
          );
          let changed_files = get_changed_files_from_hmr_edit_files(
            test_folder_path,
            &hmr_temp_dir_path,
            hmr_edit_files,
          );
          dev_engine
            .ensure_task_with_changed_files(changed_files.into_iter().map(Into::into).collect())
            .await;
        }
        drop(dev_engine);

        let mut bundle_results = std::mem::take(&mut *build_results.lock().unwrap());
        let hmr_update_infos = std::mem::take(&mut *hmr_update_infos.lock().unwrap());

        let execute_output = self.test_meta.expect_executed
          && !self.test_meta.expect_error
          && self.test_meta.write_to_disk;

        // We consider the first build output as the initial build output.
        let build_result: BuildResult<BundleOutput> = bundle_results.remove(0);

        match &build_result {
          Ok(_build_output) => {
            assert!(
              !self.test_meta.expect_error,
              "Expected the bundling to be failed with diagnosable errors, but got success"
            );

            let mut patch_chunks: Vec<String> = vec![];
            for (hmr_updates, _changed_files) in hmr_update_infos.iter().flatten() {
              for hmr_update in hmr_updates {
                match &hmr_update.update {
                  rolldown_common::HmrUpdate::Patch(patch) => {
                    let output_path = format!("{}/{}", &output_dir, &patch.filename);
                    fs::write(&output_path, &patch.code).unwrap();
                    patch_chunks.push(format!("./{}", patch.filename));
                  }
                  rolldown_common::HmrUpdate::FullReload { reason } => {
                    assert!(
                      !execute_output,
                      "execute_output should be false when full reload happens; reason: {reason:?}"
                    );
                  }
                  rolldown_common::HmrUpdate::Noop => {}
                }
              }
            }
            build_snapshot.rebuild_results = bundle_results;
            build_snapshot.hmr_updates_by_steps = hmr_update_infos;

            if execute_output {
              Self::execute_output_assets(
                &*bundler.lock().await,
                &debug_title,
                &patch_chunks,
                named_options
                  .config_name
                  .as_deref()
                  .map(Some)
                  .unwrap_or(self.test_meta.config_name.as_deref()),
              );
            } else {
              // do nothing
            }
          }
          Err(errs) => {
            assert!(
              self.test_meta.expect_error,
              "Expected the bundling to be success, but got diagnosable errors: {errs:#?}"
            );
          }
        }
        build_snapshot.initial_output = Some(build_result);
      } else {
        let mut cwd = named_options.options.cwd.clone().unwrap_or_else(|| test_folder_path.clone());
        build_snapshot.cwd = Some(cwd.clone());

        let maybe_bundler = Bundler::with_plugins(named_options.options, plugins.clone());

        match maybe_bundler {
          Ok(mut bundler) => {
            cwd.clone_from(&bundler.options().cwd);

            build_snapshot.cwd = Some(cwd.clone());

            let bundle_output = if self.test_meta.write_to_disk {
              let abs_output_dir = cwd.join(&bundler.options().out_dir);
              if abs_output_dir.is_dir() {
                std::fs::remove_dir_all(&abs_output_dir)
                  .context(format!("{abs_output_dir:?}"))
                  .expect("Failed to clean the output directory");
              }
              bundler.write().await
            } else {
              bundler.generate().await
            };

            let execute_output = self.test_meta.expect_executed
              && !self.test_meta.expect_error
              && self.test_meta.write_to_disk;

            match &bundle_output {
              Ok(_bundle_output) => {
                assert!(
                  !self.test_meta.expect_error,
                  "Expected the bundling to be failed with diagnosable errors, but got success"
                );

                if execute_output {
                  Self::execute_output_assets(
                    &bundler,
                    &debug_title,
                    &[],
                    named_options
                      .config_name
                      .as_deref()
                      .map(Some)
                      .unwrap_or(self.test_meta.config_name.as_deref()),
                  );
                } else {
                  // do nothing
                }
              }
              Err(errs) => {
                assert!(
                  self.test_meta.expect_error,
                  "Expected the bundling to be success, but got diagnosable errors: {errs:#?}"
                );
              }
            }
            build_snapshot.initial_output = Some(bundle_output);
          }
          Err(errs) => {
            assert!(
              self.test_meta.expect_error,
              "Expected the bundling to be success, but got diagnosable errors: {errs:#?}"
            );
            build_snapshot.initial_output = Some(Err(errs));
          }
        }
      }
      artifacts_snapshot.builds.push(build_snapshot);
    }
    self.snapshot_bundle_output(test_folder_path, &artifacts_snapshot.render(&self.test_meta));
  }

  fn apply_test_defaults(&self, options: &mut BundlerOptions) {
    if options.cwd.is_none() {
      options.cwd = Some(self.test_folder_path.clone());
    }

    if options.external.is_none() {
      options.external = Some(IsExternal::from(vec!["node:assert".to_string()]));
    }

    if options.input.is_none() {
      options.input = Some(vec![default_test_input_item()]);
    }

    let output_ext = "js";

    if options.entry_filenames.is_none() {
      if self.test_meta.hash_in_filename {
        options.entry_filenames = Some(format!("[name]-[hash].{output_ext}").into());
      } else {
        options.entry_filenames = Some(format!("[name].{output_ext}").into());
      }
    }

    if options.chunk_filenames.is_none() {
      if self.test_meta.hash_in_filename {
        options.chunk_filenames = Some(format!("[name]-[hash].{output_ext}").into());
      } else {
        options.chunk_filenames = Some(format!("[name].{output_ext}").into());
      }
    }

    if self.test_meta.visualize_sourcemap {
      if options.sourcemap.is_none() {
        options.sourcemap = Some(SourceMapType::File);
      } else if !matches!(options.sourcemap, Some(SourceMapType::File)) {
        panic!("`visualizeSourcemap` is only supported with `sourcemap: 'file'`")
      }
    }
    if options.sourcemap.is_none() && self.test_meta.visualize_sourcemap {
      options.sourcemap = Some(SourceMapType::File);
    }

    if let Some(experimental) = &mut options.experimental {
      if let Some(hmr) = &mut experimental.hmr {
        if hmr.implement.is_none() {
          hmr.implement = Some(include_str!("./hmr-runtime.js").to_owned());
        }
      }
    }
  }

  fn snapshot_bundle_output(&self, path: &Path, content: &str) {
    // Configure insta to use the fixture path as the snapshot path
    if self.test_meta.snapshot {
      let mut settings = insta::Settings::clone_current();
      settings.set_snapshot_path(path);
      settings.set_prepend_module_to_snapshot(false);
      settings.remove_input_file();
      settings.set_omit_expression(true);
      settings.bind(|| {
        insta::assert_snapshot!("artifacts", content);
      });
    }
  }

  fn execute_output_assets(
    bundler: &Bundler,
    test_title: &str,
    patch_chunks: &[String],
    config_name: Option<&str>,
  ) {
    let cwd = bundler.options().cwd.clone();
    let dist_folder = cwd.join(&bundler.options().out_dir);

    let is_expect_executed_under_esm = matches!(bundler.options().format, OutputFormat::Esm)
      || (!matches!(bundler.options().format, OutputFormat::Cjs)
        && matches!(bundler.options().platform, Platform::Browser));

    // add a dummy `package.json` to allow `import and export` when output module format is `esm`
    if is_expect_executed_under_esm {
      let package_json_path = dist_folder.join("package.json");
      let mut package_json = std::fs::File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .read(true)
        .open(package_json_path)
        .unwrap();
      let mut json_string = String::new();
      package_json.read_to_string(&mut json_string).unwrap();
      let mut json: Value =
        serde_json::from_str(&json_string).unwrap_or(Value::Object(Map::default()));
      json["type"] = "module".into();
      package_json.write_all(serde_json::to_string_pretty(&json).unwrap().as_bytes()).unwrap();
    }

    let test_script = cwd.join("_test.mjs");

    let mut node_command = Command::new("node");

    let globals_injection = Self::generate_globals_injection_for_execute_output(
      config_name,
      patch_chunks,
      bundler.options(),
    );

    if !globals_injection.is_empty() {
      let inject_script_url =
        format!("data:text/javascript,{}", urlencoding::encode(&globals_injection));
      node_command.arg("--import");
      node_command.arg(inject_script_url);
    }

    if test_script.exists() {
      node_command.arg(test_script);
    } else {
      // make sure to set this: https://github.com/nodejs/node/issues/59374
      node_command.arg("--input-type=module");

      let mut compiled_entries = bundler
        .options()
        .input
        .iter()
        .map(|item| {
          let name = item.name.clone().expect("inputs must have `name` in `_config.json`");
          let ext = "js";
          format!("{name}.{ext}",)
        })
        .map(|name| dist_folder.join(name))
        .map(|path| {
          if cfg!(target_os = "windows") {
            // Only URLs with a scheme in: file, data, and node are supported by the default ESM loader. On Windows, absolute paths must be valid file:// URLs.
            format!("file://{}", path.to_str().expect("should be valid utf8").replace('\\', "/"))
          } else {
            path.to_str().expect("should be valid utf8").to_string()
          }
        })
        .collect::<Vec<_>>();

      let post_globals_injection =
        Self::generate_post_globals_injection_for_execute_output(patch_chunks, &dist_folder);
      if !post_globals_injection.is_empty() {
        let inject_script_url =
          format!("data:text/javascript,{}", urlencoding::encode(&post_globals_injection));
        compiled_entries.push(inject_script_url);
      }

      node_command.arg("--eval");
      node_command.arg(
        compiled_entries
          .into_iter()
          .map(|s| format!("import '{s}';"))
          .collect::<Vec<_>>()
          .join("\n"),
      );
    }

    let output = node_command.output().unwrap();

    #[expect(clippy::print_stdout)]
    if !output.status.success() {
      let stdout_utf8 = std::str::from_utf8(&output.stdout).unwrap();
      let stderr_utf8 = std::str::from_utf8(&output.stderr).unwrap();

      println!(
        "⬇️⬇️ Failed to execute command {test_title} ⬇️⬇️\n{node_command:?}\n⬆️⬆️ end  ⬆️⬆️"
      );
      panic!(
        "⬇️⬇️ stderr {test_title} ⬇️⬇️\n{stderr_utf8}\n⬇️⬇️ stdout ⬇️⬇️\n{stdout_utf8}\n⬆️⬆️ end  ⬆️⬆️",
      );
    }
  }

  fn generate_globals_injection_for_execute_output(
    config_name: Option<&str>,
    patch_chunks: &[String],
    _options: &NormalizedBundlerOptions,
  ) -> String {
    let mut stmts = vec![];

    if let Some(config_name) = config_name {
      stmts.push(format!("globalThis.__configName = `{config_name}`;",));
    }

    if !patch_chunks.is_empty() {
      let patch_chunks_array = patch_chunks
        .iter()
        .map(|chunk| format!("\"{}\"", chunk.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
      stmts.push(format!("globalThis.__testPatches = [{patch_chunks_array}];"));
    }

    stmts.join("\n")
  }

  fn generate_post_globals_injection_for_execute_output(
    patch_chunks: &[String],
    dist_folder: &Path,
  ) -> String {
    if patch_chunks.is_empty() {
      return String::new();
    }

    format!(
      "\
import url from 'node:url';
import path from 'node:path';

const dir = '{}';
setTimeout(async () => {{
  for (const patchChunk of globalThis.__testPatches) {{
    const file = path.join(dir, patchChunk);
    try {{
      await import(url.pathToFileURL(file));
    }} catch (error) {{
      console.error('Error executing a patch:', error);
      process.exitCode = 1;
      break;
    }}
  }}
}}, 0);
      ",
      dist_folder.to_str().unwrap().replace('\\', "\\\\") // escape backslashes in Windows paths
    )
  }
}
