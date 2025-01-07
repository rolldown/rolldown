use core::str;
use std::{
  borrow::Cow,
  ffi::OsStr,
  io::{Read, Write},
  path::Path,
  process::Command,
};

use anyhow::Context;
use rolldown::{
  plugin::__inner::SharedPluginable, BundleOutput, Bundler, BundlerOptions, IsExternal,
  OutputFormat, Platform, SourceMapType,
};
use rolldown_common::Output;
use rolldown_error::{BuildDiagnostic, BuildResult, DiagnosticOptions};
use rolldown_sourcemap::SourcemapVisualizer;
use rolldown_testing_config::TestMeta;
use serde_json::{Map, Value};
use sugar_path::SugarPath;

use crate::utils::RUNTIME_MODULE_OUTPUT_RE;

#[derive(Default)]
pub struct IntegrationTest {
  test_meta: TestMeta,
}

pub struct NamedBundlerOptions {
  pub name: Option<String>,
  pub options: BundlerOptions,
}

fn default_test_input_item() -> rolldown::InputItem {
  rolldown::InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }
}

impl IntegrationTest {
  pub fn new(test_meta: TestMeta) -> Self {
    Self { test_meta }
  }

  pub async fn bundle(&self, mut options: BundlerOptions) -> BuildResult<BundleOutput> {
    self.apply_test_defaults(&mut options);

    let mut bundler = Bundler::new(options);

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

  pub async fn run_with_plugins(
    &self,
    mut options: BundlerOptions,
    plugins: Vec<SharedPluginable>,
  ) {
    self.apply_test_defaults(&mut options);

    let mut bundler = Bundler::with_plugins(options, plugins);

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

    match bundle_output {
      Ok(bundle_output) => {
        assert!(
          !self.test_meta.expect_error,
          "Expected the bundling to be failed with diagnosable errors, but got success"
        );

        self.snapshot_bundle_output(bundle_output, vec![], &cwd);

        if !self.test_meta.expect_executed
          || self.test_meta.expect_error
          || !self.test_meta.write_to_disk
        {
          // do nothing
        } else {
          Self::execute_output_assets(&bundler, "");
        }
      }
      Err(errs) => {
        assert!(
          self.test_meta.expect_error,
          "Expected the bundling to be success, but got diagnosable errors: {errs:#?}"
        );
        self.snapshot_bundle_output(BundleOutput::default(), errs.into_vec(), &cwd);
      }
    }
  }

  pub async fn run_multiple(
    &self,
    multiple_options: Vec<NamedBundlerOptions>,
    test_folder_path: &Path,
  ) {
    let mut snapshot_outputs = vec![];
    for mut named_options in multiple_options {
      self.apply_test_defaults(&mut named_options.options);

      let mut bundler = Bundler::with_plugins(named_options.options, vec![]);

      let debug_title = named_options.name.clone().unwrap_or_else(String::new);

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

      if !debug_title.is_empty() {
        snapshot_outputs.push("\n---\n\n".to_string());
        snapshot_outputs.push(format!("Variant: {debug_title}\n\n"));
      }

      match bundle_output {
        Ok(bundle_output) => {
          assert!(
            !self.test_meta.expect_error,
            "Expected the bundling to be failed with diagnosable errors, but got success"
          );

          let snapshot_content = self.render_bundle_output_to_string(bundle_output, vec![], &cwd);
          snapshot_outputs.push(snapshot_content);

          if !self.test_meta.expect_executed
            || self.test_meta.expect_error
            || !self.test_meta.write_to_disk
          {
            // do nothing
          } else {
            Self::execute_output_assets(&bundler, &debug_title);
          }
        }
        Err(errs) => {
          assert!(
            self.test_meta.expect_error,
            "Expected the bundling to be success, but got diagnosable errors: {errs:#?}"
          );
          let snapshot_content =
            self.render_bundle_output_to_string(BundleOutput::default(), errs.into_vec(), &cwd);
          snapshot_outputs.push(snapshot_content);
        }
      }
    }

    // Configure insta to use the fixture path as the snapshot path
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(test_folder_path);
    settings.set_prepend_module_to_snapshot(false);
    settings.remove_input_file();
    settings.set_omit_expression(true);
    settings.bind(|| {
      insta::assert_snapshot!("artifacts", snapshot_outputs.concat());
    });
  }

  fn apply_test_defaults(&self, options: &mut BundlerOptions) {
    if options.external.is_none() {
      options.external = Some(IsExternal::from_vec(vec!["node:assert".to_string()]));
    }

    if options.input.is_none() {
      options.input = Some(vec![default_test_input_item()]);
    }

    // if options.cwd.is_none() {
    //   options.cwd = Some(fixture_path.to_path_buf());
    // }

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
  }

  #[expect(clippy::too_many_lines)]
  #[expect(clippy::if_not_else)]
  fn render_bundle_output_to_string(
    &self,
    bundle_output: BundleOutput,
    errs: Vec<BuildDiagnostic>,
    cwd: &Path,
  ) -> String {
    let mut errors = errs;
    let errors_section = if !errors.is_empty() {
      let mut snapshot = String::new();
      snapshot.push_str("# Errors\n\n");
      errors.sort_by_key(|e| e.kind().to_string());
      let diagnostics = errors
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("## {code}\n")),
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
      snapshot.push_str("# warnings\n\n");
      let diagnostics = warnings
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));
      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("## {code}\n")),
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
      snapshot.push_str("# Assets\n\n");
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
              let content = if self.test_meta.hidden_runtime_module {
                RUNTIME_MODULE_OUTPUT_RE.replace_all(content, "")
              } else {
                Cow::Borrowed(content.as_str())
              };

              Some(vec![
                Cow::Owned(format!("## {}\n", asset.filename())),
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
                  Cow::Owned(format!("## {}\n", asset.filename())),
                  Cow::Owned(format!("```{file_ext}")),
                  Cow::Borrowed(content),
                  "```".into(),
                ]),
                rolldown_common::StrOrBytes::Bytes(bytes) => {
                  let mut ret = vec![Cow::Owned(format!("## {}\n", asset.filename()))];
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
      snapshot.push_str("## Output Stats\n\n");
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
      snapshot.push_str("# Sourcemap Visualizer\n\n");
      snapshot.push_str("```\n");
      let visualizer_result = assets
        .iter()
        .filter_map(|asset| match asset {
          Output::Chunk(chunk) => chunk.map.as_ref().map(|sourcemap| {
            SourcemapVisualizer::new(&chunk.code, sourcemap).into_visualizer_text()
          }),
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
    let snapshot = [
      errors_section,
      warnings_section,
      assets_section,
      output_stats_section,
      visualize_sourcemap_section,
    ]
    .join("\n")
    .trim()
    .to_owned();
    snapshot
  }

  fn snapshot_bundle_output(
    &self,
    bundle_output: BundleOutput,
    errs: Vec<BuildDiagnostic>,
    cwd: &Path,
  ) {
    let content = self.render_bundle_output_to_string(bundle_output, errs, cwd);
    // Configure insta to use the fixture path as the snapshot path
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(cwd);
    settings.set_prepend_module_to_snapshot(false);
    settings.remove_input_file();
    settings.set_omit_expression(true);
    settings.bind(|| {
      insta::assert_snapshot!("artifacts", content);
    });
  }

  fn execute_output_assets(bundler: &Bundler, test_title: &str) {
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

    if test_script.exists() {
      node_command.arg(test_script);
    } else {
      let compiled_entries = bundler
        .options()
        .input
        .iter()
        .map(|item| {
          let name = item.name.clone().expect("inputs must have `name` in `_config.json`");
          let ext = "js";
          format!("{name}.{ext}",)
        })
        .map(|name| dist_folder.join(name))
        .collect::<Vec<_>>();

      compiled_entries.iter().for_each(|entry| {
        node_command.arg("--import");
        if cfg!(target_os = "windows") {
          // Only URLs with a scheme in: file, data, and node are supported by the default ESM loader. On Windows, absolute paths must be valid file:// URLs.
          node_command.arg(format!("file://{}", entry.to_str().expect("should be valid utf8")));
        } else {
          node_command.arg(entry);
        }
        node_command.arg("--eval");
        node_command.arg("\"\"");
      });
    }

    let output = node_command.output().unwrap();

    #[allow(clippy::print_stdout)]
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
}
