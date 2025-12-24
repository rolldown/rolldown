use crate::BundlerOptions;
use rolldown_plugin::__inner::SharedPluginable;

/// Configuration for creating a bundler instance.
/// This is used by APIs like `Watcher` to construct bundlers internally.
#[derive(Debug)]
pub struct BundlerConfig {
  pub options: BundlerOptions,
  pub plugins: Vec<SharedPluginable>,
}

impl BundlerConfig {
  pub fn new(options: BundlerOptions, plugins: Vec<SharedPluginable>) -> Self {
    Self { options, plugins }
  }
}
