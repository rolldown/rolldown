use rolldown_error::BuildResult;
use rolldown_plugin::__inner::SharedPluginable;

use crate::{Bundler, BundlerOptions};

#[derive(Debug, Default)]
pub struct BundlerBuilder {
  options: BundlerOptions,
  plugins: Vec<SharedPluginable>,
}

impl BundlerBuilder {
  pub fn build(self) -> BuildResult<Bundler> {
    Bundler::with_builder_options(self.options, self.plugins, None, true)
  }

  #[must_use]
  pub fn with_options(mut self, options: BundlerOptions) -> Self {
    self.options = options;
    self
  }

  #[must_use]
  pub fn with_plugins(mut self, plugins: Vec<SharedPluginable>) -> Self {
    self.plugins = plugins;
    self
  }
}
