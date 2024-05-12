use std::sync::Arc;

use rolldown_fs::OsFileSystem;
use rolldown_plugin::{BoxPlugin, PluginDriver};
use rolldown_resolver::Resolver;

use crate::{
  utils::normalize_options::{normalize_options, NormalizeOptionsReturn},
  Bundler, BundlerOptions, SharedResolver,
};

#[derive(Debug, Default)]
pub struct BundlerBuilder {
  input_options: BundlerOptions,
  plugins: Vec<BoxPlugin>,
}

impl BundlerBuilder {
  pub fn build(self) -> Bundler {
    let maybe_guard = rolldown_tracing::try_init_tracing();

    let NormalizeOptionsReturn { options, resolve_options } = normalize_options(self.input_options);

    let resolver: SharedResolver =
      Resolver::new(resolve_options, options.platform, options.cwd.clone(), OsFileSystem).into();

    Bundler {
      plugin_driver: PluginDriver::new_shared(self.plugins, &resolver),
      resolver,
      options: Arc::new(options),
      fs: OsFileSystem,
      _log_guard: maybe_guard,
    }
  }

  #[must_use]
  pub fn with_options(mut self, input_options: BundlerOptions) -> Self {
    self.input_options = input_options;
    self
  }

  #[must_use]
  pub fn with_plugins(mut self, plugins: Vec<BoxPlugin>) -> Self {
    self.plugins = plugins;
    self
  }
}
