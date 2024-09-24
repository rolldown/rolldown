use std::sync::Arc;

use rolldown_common::FileEmitter;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::{PluginDriver, __inner::SharedPluginable};
use rolldown_resolver::Resolver;

use crate::{
  utils::{
    apply_inner_plugins::apply_inner_plugins,
    normalize_options::{normalize_options, NormalizeOptionsReturn},
  },
  Bundler, BundlerOptions, SharedResolver,
};

#[derive(Debug, Default)]
pub struct BundlerBuilder {
  options: BundlerOptions,
  plugins: Vec<SharedPluginable>,
}

impl BundlerBuilder {
  pub fn build(mut self) -> Bundler {
    let maybe_guard = rolldown_tracing::try_init_tracing();

    let NormalizeOptionsReturn { options, resolve_options } = normalize_options(self.options);

    let resolver: SharedResolver =
      Resolver::new(resolve_options, options.platform, options.cwd.clone(), OsFileSystem).into();

    let options = Arc::new(options);

    let file_emitter = Arc::new(FileEmitter::new(Arc::clone(&options)));

    apply_inner_plugins(&mut self.plugins);

    Bundler {
      closed: false,
      plugin_driver: PluginDriver::new_shared(self.plugins, &resolver, &file_emitter, &options),
      file_emitter,
      resolver,
      options,
      fs: OsFileSystem,
      _log_guard: maybe_guard,
    }
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
