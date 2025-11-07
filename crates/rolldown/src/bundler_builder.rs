use rolldown_error::BuildResult;
use rolldown_plugin::__inner::SharedPluginable;

use crate::{Bundler, BundlerOptions};

#[derive(Debug, Default)]
pub struct BundlerBuilder {
  options: BundlerOptions,
  plugins: Vec<SharedPluginable>,
  session: Option<rolldown_debug::Session>,
  disable_tracing_setup: bool,
  build_count: u32,
}

impl BundlerBuilder {
  pub fn build(self) -> BuildResult<Bundler> {
    Bundler::with_builder_options(
      self.options,
      self.plugins,
      self.session,
      self.disable_tracing_setup,
    )
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

  #[must_use]
  pub fn with_build_count(mut self, build_count: u32) -> Self {
    self.build_count = build_count;
    self
  }

  #[must_use]
  pub fn with_session(mut self, session: rolldown_debug::Session) -> Self {
    self.session = Some(session);
    self
  }

  #[must_use]
  pub fn with_disable_tracing_setup(mut self, disable: bool) -> Self {
    self.disable_tracing_setup = disable;
    self
  }
}
