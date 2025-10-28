use std::path::PathBuf;
use std::sync::Arc;
use std::{
  fs,
  io::{Read, Write},
  path::Path,
  process::Command,
};

use anyhow::Context;
use rolldown::dev::{DevOptions, DevWatchOptions};
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
use crate::types::{
  BuildArtifactsSnapshot, BuildRoundOutput, DevArtifactsSnapshot, DevRoundOutput, HmrStepOutput,
};

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

  /// Determine if output should be executed based on test meta
  fn should_execute_output(&self) -> bool {
    self.test_meta.expect_executed && !self.test_meta.expect_error && self.test_meta.write_to_disk
  }

  /// Run multiple bundler configurations in HMR mode
  async fn run_multiple_for_dev(
    &self,
    multiple_options: Vec<NamedBundlerOptions>,
    plugins: Vec<SharedPluginable>,
    hmr_steps: &[Vec<PathBuf>],
  ) -> DevArtifactsSnapshot {
    let test_folder_path = &self.test_folder_path;
    let hmr_temp_dir_path = test_folder_path.join("hmr-temp");
    let mut artifacts_snapshot = DevArtifactsSnapshot::default();

    for mut named_options in multiple_options {
      // Type aliases for nested callback tracking
      type HmrUpdatesBySteps = Arc<
        std::sync::Mutex<
          Vec<Vec<BuildResult<(Vec<rolldown_common::ClientHmrUpdate>, Vec<String>)>>>,
        >,
      >;
      type BuildResultsBySteps = Arc<std::sync::Mutex<Vec<Vec<BuildResult<BundleOutput>>>>>;

      let mut build_snapshot = DevRoundOutput {
        overwritten_test_meta_snapshot: named_options.snapshot.unwrap_or(self.test_meta.snapshot),
        ..Default::default()
      };

      // Setup HMR temp directory
      fs::remove_dir_all(&hmr_temp_dir_path)
        .or_else(|err| if err.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(err) })
        .unwrap();
      copy_non_hmr_edit_files_to_hmr_temp_dir(test_folder_path, &hmr_temp_dir_path);

      named_options.options.cwd = Some(hmr_temp_dir_path.clone());

      let output_dir = format!(
        "{}/{}",
        named_options.options.cwd.as_ref().map_or(".", |cwd| cwd.to_str().unwrap()),
        named_options.options.dir.as_ref().map_or("dist", |v| v)
      );

      let debug_title = named_options.description.clone().unwrap_or_else(String::new);
      if !debug_title.is_empty() {
        build_snapshot.debug_title = Some(debug_title.clone());
      }

      // Create bundler and DevEngine
      let cwd = named_options.options.cwd.clone().unwrap_or_else(|| self.test_folder_path.clone());

      let bundler_result = Bundler::with_plugins(named_options.options.clone(), plugins.clone());

      let bundler = match bundler_result {
        Ok(bundler) => {
          build_snapshot.cwd = Some(bundler.options().cwd.clone());
          bundler
        }
        Err(errs) => {
          // Set cwd and error, then skip this build round
          build_snapshot.cwd = Some(cwd);
          build_snapshot.initial_output = Some(Err(errs));
          artifacts_snapshot.builds.push(build_snapshot);
          continue;
        }
      };
      let bundler = Arc::new(Mutex::new(bundler));

      // Use nested vecs to track which step each callback belongs to
      let hmr_updates_by_steps: HmrUpdatesBySteps = Arc::new(std::sync::Mutex::new(vec![]));
      let build_results_by_steps: BuildResultsBySteps = Arc::new(std::sync::Mutex::new(vec![]));

      let dev_engine = DevEngine::with_bundler(
        Arc::clone(&bundler),
        DevOptions {
          on_hmr_updates: {
            let hmr_updates_by_steps = Arc::clone(&hmr_updates_by_steps);
            Some(Arc::new(move |result| {
              hmr_updates_by_steps
                .lock()
                .unwrap()
                .last_mut()
                .expect("Expected a vec to collect HMR outputs for current step")
                .push(result);
            }))
          },
          on_output: {
            let build_results_by_steps = Arc::clone(&build_results_by_steps);
            Some(Arc::new(move |bundle_result| {
              build_results_by_steps
                .lock()
                .unwrap()
                .last_mut()
                .expect("Expected a vec to collect build outputs for current step")
                .push(bundle_result);
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

      // Run initial build (step 0)
      build_results_by_steps.lock().unwrap().push(vec![]);
      dev_engine.run().await.unwrap();
      dev_engine.create_client_for_testing();

      // Process HMR steps
      for hmr_edit_files in hmr_steps {
        // Prepare new vecs for this step's callbacks
        hmr_updates_by_steps.lock().unwrap().push(vec![]);
        build_results_by_steps.lock().unwrap().push(vec![]);

        apply_hmr_edit_files_to_hmr_temp_dir(test_folder_path, &hmr_temp_dir_path, hmr_edit_files);
        let changed_files = get_changed_files_from_hmr_edit_files(
          test_folder_path,
          &hmr_temp_dir_path,
          hmr_edit_files,
        );
        dev_engine
          .ensure_task_with_changed_files(changed_files.into_iter().map(Into::into).collect())
          .await;

        // Optionally wait for async builds to complete
        if self.test_meta.dev.ensure_latest_build_output_for_each_step {
          dev_engine.ensure_latest_build_output().await.unwrap();
        }
      }
      drop(dev_engine);

      // Collect results
      let mut build_results_by_steps = std::mem::take(&mut *build_results_by_steps.lock().unwrap());
      let hmr_updates_by_steps = std::mem::take(&mut *hmr_updates_by_steps.lock().unwrap());

      // Extract initial build output (first build_results vec)
      let initial_build_results = build_results_by_steps.remove(0);
      let initial_build_output =
        initial_build_results.into_iter().next().expect("Expected initial build output");

      // Transform nested vecs into HmrStepOutput
      let hmr_steps_output: Vec<HmrStepOutput> = hmr_updates_by_steps
        .into_iter()
        .zip(build_results_by_steps.into_iter())
        .map(|(hmr_updates_vec, build_outputs_vec)| {
          // Each step should have exactly one HMR update callback
          let hmr_updates =
            hmr_updates_vec.into_iter().next().expect("Expected HMR update for step");

          HmrStepOutput { hmr_updates, build_outputs: build_outputs_vec }
        })
        .collect();

      // Always assign HMR steps (regardless of initial build success/failure)
      build_snapshot.hmr_steps = hmr_steps_output;

      // Verify result and process HMR if successful
      match &initial_build_output {
        Ok(_) => {
          assert!(
            !self.test_meta.expect_error,
            "Expected the bundling to be failed with diagnosable errors, but got success"
          );

          // Process HMR updates and patches for execution
          let mut patch_chunks: Vec<String> = vec![];
          for step_output in &build_snapshot.hmr_steps {
            if let Ok((client_updates, _changed_files)) = &step_output.hmr_updates {
              for hmr_update in client_updates {
                match &hmr_update.update {
                  rolldown_common::HmrUpdate::Patch(patch) => {
                    let output_path = format!("{}/{}", &output_dir, &patch.filename);
                    fs::write(&output_path, &patch.code).unwrap();
                    patch_chunks.push(format!("./{}", patch.filename));
                  }
                  rolldown_common::HmrUpdate::FullReload { reason } => {
                    assert!(
                      !self.should_execute_output(),
                      "execute_output should be false when full reload happens; reason: {reason:?}"
                    );
                  }
                  rolldown_common::HmrUpdate::Noop => {}
                }
              }
            }
          }

          // Execute output if needed
          if self.should_execute_output() {
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
          }
        }
        Err(errs) => {
          assert!(
            self.test_meta.expect_error,
            "Expected the bundling to be success, but got diagnosable errors: {errs:#?}"
          );
        }
      }

      build_snapshot.initial_output = Some(initial_build_output);
      artifacts_snapshot.builds.push(build_snapshot);
    }

    artifacts_snapshot
  }

  /// Run multiple bundler configurations in normal (non-HMR) mode
  async fn run_multiple_for_build(
    &self,
    multiple_options: Vec<NamedBundlerOptions>,
    plugins: Vec<SharedPluginable>,
  ) -> BuildArtifactsSnapshot {
    let mut artifacts_snapshot = BuildArtifactsSnapshot::default();

    for named_options in multiple_options {
      let mut build_snapshot = BuildRoundOutput {
        overwritten_test_meta_snapshot: named_options.snapshot.unwrap_or(self.test_meta.snapshot),
        ..Default::default()
      };

      let debug_title = named_options.description.clone().unwrap_or_else(String::new);
      if !debug_title.is_empty() {
        build_snapshot.debug_title = Some(debug_title.clone());
      }

      // Try to create bundler
      let cwd = named_options.options.cwd.clone().unwrap_or_else(|| self.test_folder_path.clone());

      let bundler_result = Bundler::with_plugins(named_options.options, plugins.clone());

      let mut bundler = match bundler_result {
        Ok(bundler) => {
          build_snapshot.cwd = Some(bundler.options().cwd.clone());
          bundler
        }
        Err(errs) => {
          // Set cwd and error, then skip this build round
          build_snapshot.cwd = Some(cwd);
          build_snapshot.initial_output = Some(Err(errs));
          artifacts_snapshot.builds.push(build_snapshot);
          continue;
        }
      };

      let cwd = bundler.options().cwd.clone();
      let bundle_output = if self.test_meta.write_to_disk {
        let abs_output_dir = cwd.join(&bundler.options().out_dir);
        if abs_output_dir.is_dir() {
          std::fs::remove_dir_all(&abs_output_dir)
            .context(format!("{}", abs_output_dir.display()))
            .expect("Failed to clean the output directory");
        }
        bundler.write().await
      } else {
        bundler.generate().await
      };

      // Verify result and execute output if needed
      match &bundle_output {
        Ok(_) => {
          assert!(
            !self.test_meta.expect_error,
            "Expected the bundling to be failed with diagnosable errors, but got success"
          );
          if self.should_execute_output() {
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
      artifacts_snapshot.builds.push(build_snapshot);
    }

    artifacts_snapshot
  }

  /// Dispatcher that routes to HMR or normal build based on presence of HMR edit files
  pub async fn run_multiple(
    &self,
    mut multiple_options: Vec<NamedBundlerOptions>,
    plugins: Vec<SharedPluginable>,
  ) {
    let test_folder_path = &self.test_folder_path;

    // Detect HMR mode by checking for HMR edit files
    let hmr_temp_dir_path = test_folder_path.join("hmr-temp");
    let hmr_steps = collect_hmr_edit_files(test_folder_path, &hmr_temp_dir_path);
    let hmr_mode_enabled = !hmr_steps.is_empty();

    // Apply test defaults to all options
    for named_options in &mut multiple_options {
      self.apply_test_defaults(&mut named_options.options);
    }

    // Dispatch to appropriate build method and generate snapshot
    let snapshot_content = if hmr_mode_enabled {
      let artifacts_snapshot =
        self.run_multiple_for_dev(multiple_options, plugins, &hmr_steps).await;
      artifacts_snapshot.render(&self.test_meta)
    } else {
      let artifacts_snapshot = self.run_multiple_for_build(multiple_options, plugins).await;
      artifacts_snapshot.render(&self.test_meta)
    };

    // Generate snapshot
    self.snapshot_bundle_output(test_folder_path, &snapshot_content);
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
