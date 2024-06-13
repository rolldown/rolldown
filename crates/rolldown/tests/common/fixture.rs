use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  process::Command,
};

use rolldown::{BundleOutput, Bundler, OutputFormat, SourceMapType};
use rolldown_testing::test_config::{read_test_config, TestConfig};

fn default_test_input_item() -> rolldown::InputItem {
  rolldown::InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }
}

pub struct Fixture {
  fixture_path: PathBuf,
}

impl Fixture {
  pub fn new(fixture_path: PathBuf) -> Self {
    Self { fixture_path }
  }

  #[allow(unused)]
  pub fn name(&self) -> Cow<str> {
    self.fixture_path.file_name().unwrap().to_string_lossy()
  }

  // `test.config.json` might be not exist.
  pub fn config_path(&self) -> PathBuf {
    self.fixture_path.join("_config.json")
  }

  pub fn dir_path(&self) -> &Path {
    &self.fixture_path
  }

  pub fn test_config(&self) -> TestConfig {
    read_test_config(&self.config_path())
  }

  pub fn exec(&self) {
    let test_config = self.test_config();

    let mut command = Command::new("node");

    let is_output_cjs = matches!(test_config.config.format, Some(OutputFormat::Cjs));

    let test_script = if is_output_cjs {
      self.dir_path().join("_test.cjs")
    } else {
      self.dir_path().join("_test.mjs")
    };

    if !test_config.expect_executed || test_config.expect_error {
      // do nothing
    } else {
      // Notices, we now don't have the finalized `dir` value, so we assume the `dist` folder is the output folder. But this cause
      // problem once `entry_filenames` or `dir` is configured using a different folder.
      let dist_folder = self.dir_path().join("dist");

      let compiled_entries = test_config
        .config
        .input
        .unwrap_or_else(|| vec![default_test_input_item()])
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
          command.arg("--require");
        } else {
          command.arg("--import");
        }
        if cfg!(target_os = "windows") && !is_output_cjs {
          // Only URLs with a scheme in: file, data, and node are supported by the default ESM loader. On Windows, absolute paths must be valid file:// URLs.
          command.arg(format!("file://{}", entry.to_str().expect("should be valid utf8")));
        } else {
          command.arg(entry);
        }
      });
    }

    if test_script.exists() {
      command.arg(test_script);
    } else {
      command.arg("--eval");
      command.arg("\"\"");
    }

    let output = command.output().unwrap();

    #[allow(clippy::print_stdout)]
    if !output.status.success() {
      let stdout_utf8 = std::str::from_utf8(&output.stdout).unwrap();
      let stderr_utf8 = std::str::from_utf8(&output.stderr).unwrap();

      println!("⬇️⬇️ Failed to execute command ⬇️⬇️\n{command:?}\n⬆️⬆️ end  ⬆️⬆️");
      panic!("⬇️⬇️ stderr ⬇️⬇️\n{stderr_utf8}\n⬇️⬇️ stdout ⬇️⬇️\n{stdout_utf8}\n⬆️⬆️ end  ⬆️⬆️",);
    }
  }

  pub async fn bundle(&mut self, write_to_disk: bool, with_hash: bool) -> BundleOutput {
    let fixture_path = self.dir_path();
    let test_config = self.test_config();

    let mut bundle_options = self.test_config().config;

    if bundle_options.input.is_none() {
      bundle_options.input = Some(vec![default_test_input_item()]);
    }

    if bundle_options.cwd.is_none() {
      bundle_options.cwd = Some(fixture_path.to_path_buf());
    }

    let output_ext = match bundle_options.format {
      Some(OutputFormat::Cjs) => "cjs",
      _ => "mjs",
    };

    if bundle_options.entry_filenames.is_none() {
      if with_hash {
        bundle_options.entry_filenames = Some(format!("[name]-[hash].{output_ext}"));
      } else {
        bundle_options.entry_filenames = Some(format!("[name].{output_ext}"));
      }
    }

    if bundle_options.chunk_filenames.is_none() {
      if with_hash {
        bundle_options.chunk_filenames = Some(format!("[name]-[hash].{output_ext}"));
      } else {
        bundle_options.chunk_filenames = Some(format!("[name].{output_ext}"));
      }
    }

    if test_config.visualize_sourcemap {
      if bundle_options.sourcemap.is_none() {
        bundle_options.sourcemap = Some(SourceMapType::File);
      } else if !matches!(bundle_options.sourcemap, Some(SourceMapType::File)) {
        panic!("`visualizeSourcemap` is only supported with `sourcemap: 'file'`")
      }
    }
    if bundle_options.sourcemap.is_none() && test_config.visualize_sourcemap {
      bundle_options.sourcemap = Some(SourceMapType::File);
    }

    let mut bundler = Bundler::new(bundle_options);

    if write_to_disk && fixture_path.join("dist").is_dir() {
      std::fs::remove_dir_all(fixture_path.join("dist")).unwrap();
    }
    if write_to_disk {
      bundler.write().await.unwrap()
    } else {
      bundler.generate().await.unwrap()
    }
  }
}
