use std::{fmt::Debug, sync::Arc};

use rolldown_fs::OsFileSystem;
use rolldown_plugin::{PluginDriver, PluginOrThreadSafePlugin};
use rolldown_resolver::Resolver;

use crate::{
  utils::normalize_options::{normalize_options, NormalizeOptionsReturn},
  Bundler, BundlerOptions,
};

#[derive(Debug, Default)]
pub struct BundlerBuilder {
  input_options: BundlerOptions,
  plugins: Vec<PluginOrThreadSafePlugin>,
  worker_count: u16,
}

impl BundlerBuilder {
  pub fn build(self) -> Bundler {
    rolldown_tracing::try_init_tracing();

    let NormalizeOptionsReturn { options, resolve_options } = normalize_options(self.input_options);

    Bundler {
      resolver: Resolver::new(resolve_options, options.platform, options.cwd.clone(), OsFileSystem)
        .into(),
      plugin_driver: PluginDriver::new_shared(self.plugins, self.worker_count),
      options: Arc::new(options),
      fs: OsFileSystem,
    }
  }

  #[must_use]
  pub fn with_options(mut self, input_options: BundlerOptions) -> Self {
    self.input_options = input_options;
    self
  }

  #[must_use]
  pub fn with_plugins(mut self, plugins: Vec<PluginOrThreadSafePlugin>) -> Self {
    self.plugins = plugins;
    self
  }

  #[must_use]
  pub fn with_worker_count(mut self, worker_count: u16) -> Self {
    self.worker_count = worker_count;
    self
  }
}
