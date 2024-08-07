use std::{borrow::Cow, path::Path, process::Command};

use anyhow::Context;
use rolldown::{
  plugin::__inner::SharedPluginable, BundleOutput, Bundler, BundlerOptions, IsExternal,
  OutputFormat, SourceMapType,
};
use rolldown_common::Output;
use rolldown_error::DiagnosticOptions;
use rolldown_sourcemap::SourcemapVisualizer;
use rolldown_testing_config::TestMeta;
use sugar_path::SugarPath;

use crate::utils::RUNTIME_MODULE_OUTPUT_RE;

#[derive(Default)]
pub struct IntegrationTest {
  test_meta: TestMeta,
}

fn default_test_input_item() -> rolldown::InputItem {
  rolldown::InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }
}

impl IntegrationTest {
  pub fn new(test_meta: TestMeta) -> Self {
    Self { test_meta }
  }

  pub async fn bundle(&self, mut options: BundlerOptions) -> BundleOutput {
    self.apply_test_defaults(&mut options);

    let mut bundler = Bundler::new(options);

    if self.test_meta.write_to_disk {
      if bundler.options().dir.as_path().is_dir() {
        std::fs::remove_dir_all(&bundler.options().dir)
          .context(bundler.options().dir.clone())
          .expect("Failed to clean the output directory");
      }
      bundler.write().await.unwrap()
    } else {
      bundler.generate().await.unwrap()
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
      let abs_output_dir = cwd.join(&bundler.options().dir);
      if abs_output_dir.is_dir() {
        std::fs::remove_dir_all(&abs_output_dir)
          .context(format!("{abs_output_dir:?}"))
          .expect("Failed to clean the output directory");
      }
      bundler.write().await.unwrap()
    } else {
      bundler.generate().await.unwrap()
    };

    assert!(
      !(self.test_meta.expect_error && bundle_output.errors.is_empty()),
      "Expected the bundling to be failed with diagnosable errors, but got success"
    );

    assert!(
      !(!bundle_output.errors.is_empty() && !self.test_meta.expect_error),
      "Expected the bundling to be success, but got diagnosable errors: {:?}",
      bundle_output.errors
    );

    self.snapshot_bundle_output(bundle_output, &cwd);

    if !self.test_meta.expect_executed
      || self.test_meta.expect_error
      || !self.test_meta.write_to_disk
    {
      // do nothing
    } else {
      Self::execute_output_assets(&bundler);
    }
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

    let output_ext = match options.format {
      Some(OutputFormat::Cjs) => "cjs",
      _ => "mjs",
    };

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

  #[allow(clippy::too_many_lines)]
  fn snapshot_bundle_output(&self, bundle_output: BundleOutput, cwd: &Path) {
    let mut snapshot = String::new();
    let mut errors = bundle_output.errors;
    if !errors.is_empty() {
      snapshot.push_str("# Errors\n\n");
      errors.sort_by_key(|e| e.kind().to_string());
      let diagnostics = errors
        .into_iter()
        .map(|e| (e.kind(), e.into_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

      let rendered = diagnostics
        .flat_map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("## {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
        })
        .collect::<Vec<_>>()
        .join("\n");
      snapshot.push_str(&rendered);
    }

    // Make the snapshot consistent
    let mut warnings = bundle_output.warnings;
    if !warnings.is_empty() {
      warnings.sort_by(|a, b| {
        let a = a.to_string();
        let b = b.to_string();
        a.cmp(&b)
      });
      snapshot.push_str("# warnings\n\n");
      let diagnostics = warnings
        .into_iter()
        .map(|e| (e.kind(), e.into_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));
      let rendered = diagnostics
        .flat_map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("## {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
        })
        .collect::<Vec<_>>()
        .join("\n");
      snapshot.push_str(&rendered);
      snapshot.push('\n');
    }

    let mut assets = bundle_output.assets;

    if !assets.is_empty() {
      snapshot.push_str("# Assets\n\n");
      assets.sort_by_key(|c| c.filename().to_string());
      let artifacts = assets
        .iter()
        .filter(|asset| {
          !asset.filename().contains("$runtime$") && matches!(asset, Output::Chunk(_))
        })
        .flat_map(|asset| {
          let content = std::str::from_utf8(asset.content_as_bytes()).unwrap();
          let content = if self.test_meta.hidden_runtime_module {
            RUNTIME_MODULE_OUTPUT_RE.replace_all(content, "")
          } else {
            Cow::Borrowed(content)
          };

          [Cow::Owned(format!("## {}\n", asset.filename())), "```js".into(), content, "```".into()]
        })
        .collect::<Vec<_>>()
        .join("\n");
      snapshot.push_str(&artifacts);
    }

    if self.test_meta.snapshot_output_stats {
      snapshot.push_str("\n\n## Output Stats\n\n");
      let stats = assets
        .iter()
        .flat_map(|asset| match asset {
          Output::Chunk(chunk) => {
            vec![Cow::Owned(format!(
              "- {}, is_entry {}, is_dynamic_entry {}, exports {:?}",
              chunk.filename.as_str(),
              chunk.is_entry,
              chunk.is_dynamic_entry,
              chunk.exports
            ))]
          }
          Output::Asset(_) => vec![],
        })
        .collect::<Vec<_>>()
        .join("\n");
      snapshot.push_str(&stats);
    }

    if self.test_meta.visualize_sourcemap {
      snapshot.push_str("\n\n# Sourcemap Visualizer\n\n");
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
    }

    // Configure insta to use the fixture path as the snapshot path
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(cwd);
    settings.set_prepend_module_to_snapshot(false);
    settings.remove_input_file();
    settings.set_omit_expression(true);
    settings.bind(|| {
      insta::assert_snapshot!("artifacts", snapshot);
    });
  }

  fn execute_output_assets(bundler: &Bundler) {
    let cwd = bundler.options().cwd.clone();
    let dist_folder = cwd.join(&bundler.options().dir);
    let is_output_cjs = matches!(bundler.options().format, OutputFormat::Cjs);

    let test_script = if is_output_cjs { cwd.join("_test.cjs") } else { cwd.join("_test.mjs") };

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
          let ext = if is_output_cjs { "cjs" } else { "mjs" };
          format!("{name}.{ext}",)
        })
        .map(|name| dist_folder.join(name))
        .collect::<Vec<_>>();

      compiled_entries.iter().for_each(|entry| {
        if is_output_cjs {
          node_command.arg("--require");
        } else {
          node_command.arg("--import");
        }
        if cfg!(target_os = "windows") && !is_output_cjs {
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

      println!("⬇️⬇️ Failed to execute command ⬇️⬇️\n{node_command:?}\n⬆️⬆️ end  ⬆️⬆️");
      panic!("⬇️⬇️ stderr ⬇️⬇️\n{stderr_utf8}\n⬇️⬇️ stdout ⬇️⬇️\n{stdout_utf8}\n⬆️⬆️ end  ⬆️⬆️",);
    }
  }
}
