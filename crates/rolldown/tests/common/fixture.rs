use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  process::Command,
};

use rolldown::{BundleOutput, Bundler, SourceMapType};
use rolldown_testing::TestConfig;

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
    TestConfig::from_config_path(&self.config_path())
  }

  pub fn exec(&self) {
    let test_config = self.test_config();

    if !test_config.expect_executed || test_config.expect_error {
      return;
    }

    let dist_folder = self.dir_path().join("dist");
    let test_script = self.dir_path().join("_test.mjs");

    let compiled_entries = test_config
      .config
      .input
      .unwrap_or_else(|| vec![default_test_input_item()])
      .iter()
      .map(|item| {
        format!("{}.mjs", item.name.clone().expect("inputs must have `name` in `_config.json`"))
      })
      .map(|name| dist_folder.join(name))
      .collect::<Vec<_>>();

    let mut command = Command::new("node");
    compiled_entries.iter().for_each(|entry| {
      command.arg("--import");
      if cfg!(target_os = "windows") {
        // Only URLs with a scheme in: file, data, and node are supported by the default ESM loader. On Windows, absolute paths must be valid file:// URLs.
        command.arg(format!("file://{}", entry.to_str().expect("should be valid utf8")));
      } else {
        command.arg(entry);
      }
    });

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

  pub async fn compile(&mut self) -> BundleOutput {
    let fixture_path = self.dir_path();
    let test_config = self.test_config();

    let mut bundle_options = self.test_config().config;

    if bundle_options.input.is_none() {
      bundle_options.input = Some(vec![default_test_input_item()]);
    }

    if bundle_options.cwd.is_none() {
      bundle_options.cwd = Some(fixture_path.to_path_buf());
    }

    if bundle_options.entry_file_names.is_none() {
      bundle_options.entry_file_names = Some("[name].mjs".to_string());
    }
    if bundle_options.chunk_file_names.is_none() {
      bundle_options.chunk_file_names = Some("[name].mjs".to_string());
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

    if fixture_path.join("dist").is_dir() {
      std::fs::remove_dir_all(fixture_path.join("dist")).unwrap();
    }

    bundler.write().await
  }
}
