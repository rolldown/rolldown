use std::sync::Arc;

use rolldown_common::FileEmitter;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::{PluginDriver, SharedPlugin};
use rolldown_resolver::Resolver;

use crate::{
  utils::normalize_options::{normalize_options, NormalizeOptionsReturn},
  Bundler, BundlerOptions, SharedResolver,
};

#[derive(Debug, Default)]
pub struct BundlerBuilder {
  input_options: BundlerOptions,
  plugins: Vec<SharedPlugin>,
}

impl BundlerBuilder {
  pub fn build(self) -> Bundler {
    let maybe_guard = rolldown_tracing::try_init_tracing();

    let NormalizeOptionsReturn { options, resolve_options } = normalize_options(self.input_options);

    let resolver: SharedResolver =
      Resolver::new(resolve_options, options.platform, options.cwd.clone(), OsFileSystem).into();

    let options = Arc::new(options);

    let file_emitter = Arc::new(FileEmitter::new(Arc::clone(&options)));

    Bundler {
      plugin_driver: PluginDriver::new_shared(self.plugins, &resolver, &file_emitter),
      file_emitter,
      resolver,
      options,
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
  pub fn with_plugins(mut self, plugins: Vec<SharedPlugin>) -> Self {
    self.plugins = plugins;
    self
  }
}
