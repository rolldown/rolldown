use std::sync::Arc;

use rolldown_common::{Cache, FileEmitter, NormalizedBundlerOptions};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::{PluginDriver, __inner::SharedPluginable};
use rolldown_resolver::{ResolveError, Resolver};

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

    let NormalizeOptionsReturn { mut options, resolve_options, warnings } =
      normalize_options(self.options);
    let tsconfig_filename = resolve_options.tsconfig_filename.clone();
    let resolver: SharedResolver =
      Resolver::new(resolve_options, options.platform, options.cwd.clone(), OsFileSystem).into();

    _ = Self::merge_transform_config_from_ts_config(&mut options, tsconfig_filename, &resolver);

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
      warnings,
      _log_guard: maybe_guard,
      cache: Arc::new(Cache::default()),
    }
  }

  fn merge_transform_config_from_ts_config(
    _options: &mut NormalizedBundlerOptions,
    tsconfig_filename: Option<String>,
    resolver: &SharedResolver,
  ) -> Result<(), ResolveError> {
    let Some(tsconfig_filename) = tsconfig_filename else {
      return Ok(());
    };
    resolver.resolve_tsconfig(&tsconfig_filename)?;
    // TODO: Implement the rest of this function needs oxc_resolver new release
    Ok(())
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
