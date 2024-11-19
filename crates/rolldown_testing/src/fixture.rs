use std::path::{Path, PathBuf};

use crate::{
  integration_test::{IntegrationTest, NamedBundlerOptions},
  test_config::read_test_config,
};

use rolldown_testing_config::TestConfig;

pub struct Fixture {
  config_path: PathBuf,
  fixture_path: PathBuf,
}

impl Fixture {
  pub fn new(path: impl AsRef<Path>) -> Self {
    // Paths could be UNC format in windows, see https://github.com/rust-lang/rust/issues/42869 for more details
    let path = dunce::simplified(path.as_ref());

    Self { fixture_path: path.to_path_buf(), config_path: path.join("_config.json") }
  }

  pub fn run_integration_test(self) {
    tokio::runtime::Runtime::new().unwrap().block_on(self.run_inner());
  }

  async fn run_inner(self) {
    let TestConfig { config: mut options, meta, config_variants } =
      read_test_config(&self.config_path);

    if options.cwd.is_none() {
      options.cwd = Some(self.fixture_path.clone());
    }

    let configs = [NamedBundlerOptions { options: options.clone(), name: None }]
      .into_iter()
      .chain(config_variants.into_iter().map(|variant| NamedBundlerOptions {
        options: variant.apply(&options),
        name: Some(variant.to_string()),
      }))
      .collect::<Vec<_>>();

    IntegrationTest::new(meta).run_multiple(configs, &self.fixture_path).await;
  }
}
