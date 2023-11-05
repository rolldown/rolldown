use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
};

use rolldown::{Asset, Bundler, InputOptions, OutputOptions};
use rolldown_error::BuildError;
use rolldown_fs::{FileSystem, FileSystemOs};
use rolldown_testing::TestConfig;

fn default_test_input_item() -> rolldown_testing::InputItem {
  rolldown_testing::InputItem { name: "main".to_string(), import: "./main.js".to_string() }
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
    self.fixture_path.join("test.config.json")
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
      .input
      .input
      .unwrap_or(vec![default_test_input_item()])
      .iter()
      .map(|item| format!("{}.mjs", item.name))
      .map(|name| dist_folder.join(name))
      .collect::<Vec<_>>();

    let mut command = Command::new("node");
    compiled_entries.iter().for_each(|entry| {
      command.arg("--import");
      command.arg(entry);
    });

    if test_script.exists() {
      command.arg(test_script);
    } else {
      command.arg("--eval");
      command.arg("\"\"");
    }

    let output = command.output().unwrap();
    if !output.status.success() {
      let stdout_utf8 = std::str::from_utf8(&output.stdout).unwrap();
      let stderr_utf8 = std::str::from_utf8(&output.stderr).unwrap();
      println!("⬇️⬇️ Failed to execute command ⬇️⬇️\n{command:?}\n⬆️⬆️ end  ⬆️⬆️");
      panic!("⬇️⬇️ stderr ⬇️⬇️\n{stderr_utf8}\n⬇️⬇️ stdout ⬇️⬇️\n{stdout_utf8}\n⬆️⬆️ end  ⬆️⬆️",);
    }
  }

  pub async fn compile(&mut self) -> Result<Vec<Asset>, Vec<BuildError>> {
    let fixture_path = self.dir_path();

    let mut test_config = self.test_config();

    if test_config.input.input.is_none() {
      test_config.input.input = Some(vec![default_test_input_item()])
    }

    let mut bundler = Bundler::new(InputOptions {
      input: test_config.input.input.map(|items| {
        items
          .into_iter()
          .map(|item| rolldown::InputItem { name: Some(item.name), import: item.import })
          .collect()
      }),
      cwd: Some(fixture_path.to_path_buf()),
      fs: Arc::new(FileSystemOs),
    });

    if fixture_path.join("dist").is_dir() {
      std::fs::remove_dir_all(fixture_path.join("dist")).unwrap();
    }

    bundler
      .write(OutputOptions {
        entry_file_names: Some("[name].mjs".to_string()),
        chunk_file_names: Some("[name].mjs".to_string()),
        ..Default::default()
      })
      .await
  }
}
