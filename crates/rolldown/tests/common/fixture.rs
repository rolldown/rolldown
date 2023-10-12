use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use rolldown::{Asset, Bundler, InputItem, InputOptions, OutputOptions};
use rolldown_testing::TestConfig;

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

  pub async fn compile(&mut self) -> Vec<Asset> {
    let fixture_path = self.dir_path();

    let mut test_config = TestConfig::from_config_path(&self.config_path());

    if test_config.input.input.is_none() {
      test_config.input.input = Some(vec![rolldown_testing::InputItem {
        name: "main".to_string(),
        import: "./main.js".to_string(),
      }])
    }

    let mut bundler = Bundler::new(InputOptions {
      input: test_config.input.input.map(|items| {
        items
          .into_iter()
          .map(|item| InputItem {
            name: Some(item.name),
            import: item.import,
          })
          .collect()
      }),
      cwd: Some(fixture_path.to_path_buf()),
    });

    if fixture_path.join("dist").is_dir() {
      std::fs::remove_dir_all(fixture_path.join("dist")).unwrap();
    }

    let output = bundler
      .write(OutputOptions {
        manual_chunks: test_config.output.manual_chunks,
        // dir: Some(fixture_path.join("dist").to_string_lossy().to_string()),
        // format: ModuleFormat::from_str(&tester.config.output.format).unwrap(),
        // export_mode: ExportMode::from_str(&tester.config.output.export_mode).unwrap(),
        ..Default::default()
      })
      .await
      .unwrap();

    output
  }
}
