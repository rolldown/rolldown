use std::{
  path::{Path, PathBuf},
  sync::OnceLock,
};

use crate::{
  integration_test::{IntegrationTest, NamedBundlerOptions},
  test_config::read_test_config,
};
use rolldown::{BundlerOptions, plugin::__inner::SharedPluginable};
use rolldown_testing_config::{ConfigVariant, TestConfig, TestMeta};

pub struct Fixture {
  config_path: PathBuf,
  fixture_path: PathBuf,
}

// Using std once lock to store env variable
static NEEDS_EXTENDED_TESTS: OnceLock<bool> = OnceLock::new();
/// A function to get the API key.
///
/// This is the idiomatic way to wrap the OnceLock, providing a clean,
/// easy-to-use interface.
fn needs_extended_tests() -> bool {
  *NEEDS_EXTENDED_TESTS
    .get_or_init(|| std::env::var("NEEDS_EXTENDED").ok().unwrap_or("true".to_owned()) == "true")
}

impl Fixture {
  pub fn new(path: impl AsRef<Path>) -> Self {
    // Paths could be UNC format in windows, see https://github.com/rust-lang/rust/issues/42869 for more details
    let path = dunce::simplified(path.as_ref());

    Self { fixture_path: path.to_path_buf(), config_path: path.join("_config.json") }
  }

  pub fn run_integration_test(self) {
    tokio::runtime::Runtime::new().unwrap().block_on(self.run_inner(vec![]));
  }

  pub fn run_integration_test_with_plugins(self, plugins: Vec<SharedPluginable>) {
    tokio::runtime::Runtime::new().unwrap().block_on(self.run_inner(plugins));
  }

  async fn run_inner(self, plugins: Vec<SharedPluginable>) {
    let TestConfig { config: mut options, meta, mut config_variants } =
      read_test_config(&self.config_path);

    if options.cwd.is_none() {
      options.cwd = Some(self.fixture_path.clone());
    }

    if needs_extended_tests() {
      Self::apply_extended_tests(&meta, &options, &mut config_variants);
    }

    let configs = std::iter::once(NamedBundlerOptions {
      options: options.clone(),
      description: None,
      snapshot: None,
      config_name: None,
    })
    .chain(config_variants.into_iter().map(|variant| NamedBundlerOptions {
      options: variant.apply(&options),
      description: Some(variant.description()),
      snapshot: variant.snapshot,
      config_name: variant.config_name,
    }))
    .collect::<Vec<_>>();

    IntegrationTest::new(meta, self.fixture_path.clone()).run_multiple(configs, plugins).await;
  }

  fn apply_extended_tests(
    meta: &TestMeta,
    options: &BundlerOptions,
    config_variants: &mut Vec<ConfigVariant>,
  ) {
    if meta.extended_tests.minify_internal_exports && options.minify_internal_exports.is_none() {
      config_variants.push(ConfigVariant {
        config_name: Some("extended-minify-internal-exports".to_string()),
        minify_internal_exports: Some(true),
        snapshot: Some(false),
        ..Default::default()
      });
    }
  }
}
