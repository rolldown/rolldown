use core::str;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
  borrow::Cow,
  ffi::OsStr,
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
use rolldown_common::{HmrUpdate, Output};
use rolldown_error::{BuildDiagnostic, BuildResult, DiagnosticOptions};
use rolldown_sourcemap::SourcemapVisualizer;
use rolldown_testing_config::TestMeta;
use serde_json::{Map, Value};
use sugar_path::SugarPath;
use tokio::sync::Mutex;

use crate::hmr_files::{
  apply_hmr_edit_files_to_hmr_temp_dir, collect_hmr_edit_files,
  copy_non_hmr_edit_files_to_hmr_temp_dir, get_changed_files_from_hmr_edit_files,
};
use crate::utils::tweak_snapshot;

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

  #[expect(clippy::too_many_lines)]
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

    let mut snapshot_outputs = vec![];
    for mut named_options in multiple_options {
      self.apply_test_defaults(&mut named_options.options);
      let allow_to_collect_snapshot = named_options.snapshot.unwrap_or(self.test_meta.snapshot);
      let mut collect_snapshot = |content: String| {
        if allow_to_collect_snapshot {
          snapshot_outputs.push(content);
        }
      };

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
        collect_snapshot("\n---\n\n".to_string());
        collect_snapshot(format!("Variant: {debug_title}\n\n"));
      }

      if hmr_mode_enabled {
        let options = named_options.options.clone();
        // FIXME: hyf0 we shouln't use the same bundler for both hmr and non-hmr mode.
        let bundler =
          Bundler::with_plugins(options, plugins.clone()).expect("Failed to create bundler");
        let cwd = bundler.options().cwd.clone();
        let bundler = Arc::new(Mutex::new(bundler));

        let has_full_reload_update = Arc::new(AtomicBool::new(false));
        let hmr_update_infos = Arc::new(std::sync::Mutex::new(vec![]));
        let build_outputs = Arc::new(std::sync::Mutex::new(vec![]));
        let dev_engine = DevEngine::with_bundler(
          Arc::clone(&bundler),
          DevOptions {
            on_hmr_updates: {
              let hmr_update_infos = Arc::clone(&hmr_update_infos);
              let has_full_reload_update = Arc::clone(&has_full_reload_update);
              Some(Arc::new(move |updates, changed_files| {
                let is_full_reload = updates.iter().any(|update| update.update.is_full_reload());
                if is_full_reload {
                  has_full_reload_update.store(true, Ordering::SeqCst);
                }
                hmr_update_infos.lock().unwrap().push((updates, changed_files));
              }))
            },
            on_output: {
              let build_outputs = Arc::clone(&build_outputs);
              Some(Arc::new(move |bundle_output| {
                build_outputs.lock().unwrap().push(bundle_output);
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
        dev_engine.create_client_for_testing().await;

        for hmr_edit_files in &hmr_steps {
          // Reset the flag
          has_full_reload_update.store(false, Ordering::SeqCst);

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
          if has_full_reload_update.load(Ordering::SeqCst) {
            dev_engine.ensure_latest_build_output().await.unwrap();
          }
        }
        drop(dev_engine);

        let mut build_outputs = std::mem::take(&mut *build_outputs.lock().unwrap());
        let hmr_update_infos = std::mem::take(&mut *hmr_update_infos.lock().unwrap());

        let execute_output = self.test_meta.expect_executed
          && !self.test_meta.expect_error
          && self.test_meta.write_to_disk;

        let bundle_output: BuildResult<BundleOutput> = Ok(build_outputs.remove(0));

        match bundle_output {
          Ok(bundle_output) => {
            assert!(
              !self.test_meta.expect_error,
              "Expected the bundling to be failed with diagnosable errors, but got success"
            );

            let snapshot_content =
              self.render_bundle_output_to_string(bundle_output, vec![], &cwd, 0);
            collect_snapshot(snapshot_content);

            let mut patch_chunks: Vec<String> = vec![];
            for (step, (hmr_updates, _changed_files)) in hmr_update_infos.iter().enumerate() {
              for hmr_update in hmr_updates {
                let snapshot_content = self.render_hmr_output_to_string(
                  step,
                  &hmr_update.update,
                  vec![],
                  &mut build_outputs,
                  &cwd,
                );
                collect_snapshot(snapshot_content);
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
            let snapshot_content = self.render_bundle_output_to_string(
              BundleOutput::default(),
              errs.into_vec(),
              &cwd,
              0,
            );
            collect_snapshot(snapshot_content);
          }
        }
      } else {
        let mut bundler = Bundler::with_plugins(named_options.options, plugins.clone())
          .expect("Failed to create bundler");

        let cwd = bundler.options().cwd.clone();

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

        match bundle_output {
          Ok(bundle_output) => {
            assert!(
              !self.test_meta.expect_error,
              "Expected the bundling to be failed with diagnosable errors, but got success"
            );

            let snapshot_content =
              self.render_bundle_output_to_string(bundle_output, vec![], &cwd, 0);
            collect_snapshot(snapshot_content);

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
            let snapshot_content = self.render_bundle_output_to_string(
              BundleOutput::default(),
              errs.into_vec(),
              &cwd,
              0,
            );
            collect_snapshot(snapshot_content);
          }
        }
      }
    }
    self.snapshot_bundle_output(test_folder_path, &snapshot_outputs.concat());
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

  #[expect(clippy::too_many_lines)]
  #[expect(clippy::if_not_else)]
  fn render_bundle_output_to_string(
    &self,
    bundle_output: BundleOutput,
    errs: Vec<BuildDiagnostic>,
    cwd: &Path,
    heading_level: usize,
  ) -> String {
    let heading_prefix = "#".repeat(heading_level);
    let mut errors = errs;
    let errors_section = if !errors.is_empty() {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}# Errors\n\n").unwrap();
      errors.sort_by_key(|e| e.kind().to_string());
      let diagnostics = errors
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("{heading_prefix}## {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
          .join("\n")
        })
        .collect::<Vec<_>>();
      rendered_diagnostics.sort();
      let rendered = rendered_diagnostics.join("\n");
      snapshot.push_str(&rendered);
      snapshot
    } else {
      String::default()
    };

    let warnings = bundle_output.warnings;
    let warnings_section = if !warnings.is_empty() {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}# warnings\n\n").unwrap();
      let diagnostics = warnings
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));
      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("{heading_prefix}## {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
          .join("\n")
        })
        .collect::<Vec<_>>();

      // Make the snapshot consistent
      rendered_diagnostics.sort();
      snapshot.push_str(&rendered_diagnostics.join("\n"));
      snapshot
    } else {
      String::new()
    };

    let mut assets = bundle_output.assets;

    let assets_section = if !assets.is_empty() {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}# Assets\n\n").unwrap();
      assets.sort_by_key(|c| c.filename().to_string());
      let artifacts = assets
        .iter()
        .filter_map(|asset| {
          let filename = asset.filename();
          let file_ext = filename.as_path().extension().and_then(OsStr::to_str).map_or(
            "unknown",
            |ext| match ext {
              "mjs" | "cjs" => "js",
              _ => ext,
            },
          );

          match asset {
            Output::Chunk(output_chunk) => {
              let content = &output_chunk.code;
              let content = tweak_snapshot(content, self.test_meta.hidden_runtime_module, true);

              Some(vec![
                Cow::Owned(format!("{heading_prefix}## {}\n", asset.filename())),
                Cow::Owned(format!("```{file_ext}")),
                content,
                "```".into(),
              ])
            }
            Output::Asset(output_asset) => {
              if file_ext == "map" {
                // Skip sourcemap for now
                return None;
              }
              match &output_asset.source {
                rolldown_common::StrOrBytes::Str(content) => Some(vec![
                  Cow::Owned(format!("{heading_prefix}## {}\n", asset.filename())),
                  Cow::Owned(format!("```{file_ext}")),
                  Cow::Borrowed(content),
                  "```".into(),
                ]),
                rolldown_common::StrOrBytes::Bytes(bytes) => {
                  let mut ret =
                    vec![Cow::Owned(format!("{heading_prefix}## {}\n", asset.filename()))];
                  if self.test_meta.snapshot_bytes {
                    ret.extend([
                      Cow::Owned(format!("```{file_ext}")),
                      String::from_utf8_lossy(bytes),
                      "```".into(),
                    ]);
                  }
                  Some(ret)
                }
              }
            }
          }
        })
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
      snapshot.push_str(&artifacts);
      snapshot
    } else {
      String::new()
    };

    let output_stats_section = if self.test_meta.snapshot_output_stats {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}## Output Stats\n\n").unwrap();
      let stats = assets
        .iter()
        .flat_map(|asset| match asset {
          Output::Chunk(chunk) => {
            vec![Cow::Owned(format!(
              "- {}, is_entry {}, is_dynamic_entry {}, exports {:?}",
              chunk.filename.as_str(),
              chunk.is_entry,
              chunk.is_dynamic_entry,
              chunk.exports.iter().map(ToString::to_string).collect::<Vec<_>>()
            ))]
          }
          Output::Asset(_) => vec![],
        })
        .collect::<Vec<_>>()
        .join("\n");
      snapshot.push_str(&stats);
      snapshot
    } else {
      String::new()
    };

    let visualize_sourcemap_section = if self.test_meta.visualize_sourcemap {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}# Sourcemap Visualizer\n\n").unwrap();
      snapshot.push_str("```\n");
      let visualizer_result = assets
        .iter()
        .filter_map(|asset| match asset {
          Output::Chunk(chunk) => chunk
            .map
            .as_ref()
            .map(|sourcemap| SourcemapVisualizer::new(&chunk.code, sourcemap).get_text()),
          Output::Asset(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
      snapshot.push_str(&visualizer_result);
      snapshot.push_str("```");
      snapshot
    } else {
      String::new()
    };
    [
      errors_section,
      warnings_section,
      assets_section,
      output_stats_section,
      visualize_sourcemap_section,
    ]
    .join("\n")
    .trim()
    .to_owned()
  }

  #[expect(clippy::if_not_else)]
  fn render_hmr_output_to_string(
    &self,
    step: usize,
    hmr_update: &HmrUpdate,
    errs: Vec<BuildDiagnostic>,
    build_outputs: &mut Vec<BundleOutput>,
    cwd: &Path,
  ) -> String {
    let mut errors = errs;
    let errors_section = if !errors.is_empty() {
      let mut snapshot = String::new();
      snapshot.push_str("## Errors\n\n");
      errors.sort_by_key(|e| e.kind().to_string());
      let diagnostics = errors
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("### {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
          .join("\n")
        })
        .collect::<Vec<_>>();
      rendered_diagnostics.sort();
      let rendered = rendered_diagnostics.join("\n");
      snapshot.push_str(&rendered);
      snapshot
    } else {
      String::default()
    };

    let code_section = match hmr_update {
      HmrUpdate::Patch(hmr_patch) if !hmr_patch.code.is_empty() => {
        let mut snapshot = String::new();
        write!(snapshot, "## Code\n\n").unwrap();
        let file_ext = hmr_patch.filename.as_path().extension().and_then(OsStr::to_str).map_or(
          "unknown",
          |ext| match ext {
            "mjs" | "cjs" => "js",
            _ => ext,
          },
        );
        writeln!(snapshot, "```{file_ext}").unwrap();
        snapshot.push_str(&tweak_snapshot(
          &hmr_patch.code,
          self.test_meta.hidden_runtime_module,
          true,
        ));
        snapshot.push_str("\n```");
        snapshot
      }
      HmrUpdate::FullReload { .. } => {
        let bundle_output = build_outputs.remove(0);
        self.render_bundle_output_to_string(bundle_output, vec![], cwd, 1)
      }
      _ => String::new(),
    };

    let meta_section = {
      let mut snapshot = String::new();
      snapshot.push_str("## Meta\n\n");
      writeln!(
        snapshot,
        "- update type: {}",
        match hmr_update {
          HmrUpdate::Patch(_) => "patch",
          HmrUpdate::FullReload { .. } => "full-reload",
          HmrUpdate::Noop => "noop",
        }
      )
      .unwrap();

      match hmr_update {
        HmrUpdate::Patch(hmr_patch) => {
          write!(snapshot, "### Hmr Boundaries\n\n").unwrap();
          let meta = hmr_patch
            .hmr_boundaries
            .iter()
            .map(|boundary| {
              format!(
                "- boundary: {}, accepted_via: {}",
                boundary.boundary.as_str(),
                boundary.accepted_via.as_str()
              )
            })
            .collect::<Vec<_>>();
          snapshot.push_str(&meta.join("\n"));
        }
        HmrUpdate::FullReload { reason } => {
          writeln!(snapshot, "- reason: {reason}").unwrap();
        }
        HmrUpdate::Noop => {}
      }

      snapshot
    };

    "\n".to_owned()
      + [format!("# HMR Step {step}"), errors_section, code_section, meta_section].join("\n").trim()
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
