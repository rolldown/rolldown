use std::sync::Arc;

use rolldown_common::{Cache, FileEmitter, NormalizedBundlerOptions};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::{__inner::SharedPluginable, PluginDriver};
use rolldown_resolver::{ResolveError, Resolver};

use crate::{
  Bundler, BundlerOptions, SharedResolver,
  utils::{
    apply_inner_plugins::apply_inner_plugins,
    normalize_options::{NormalizeOptionsReturn, normalize_options},
  },
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

    // TODO: error handling
    Self::merge_transform_config_from_ts_config(&mut options, tsconfig_filename, &resolver)
      .unwrap();

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
      hmr_manager: None,
    }
  }

  fn merge_transform_config_from_ts_config(
    options: &mut NormalizedBundlerOptions,
    tsconfig_filename: Option<String>,
    resolver: &SharedResolver,
  ) -> Result<(), ResolveError> {
    let Some(tsconfig_filename) = tsconfig_filename else {
      return Ok(());
    };
    let ts_config = resolver.resolve_tsconfig(&options.cwd.join(tsconfig_filename))?;
    if let Some(ref jsx_factory) = ts_config.compiler_options.jsx_factory {
      options.transform_options.jsx.pragma = Some(jsx_factory.clone());
    }

    if let Some(ref jsx_fragment_factory) = ts_config.compiler_options.jsx_fragment_factory {
      options.transform_options.jsx.pragma_frag = Some(jsx_fragment_factory.clone());
    }

    if let Some(ref jsx_import_source) = ts_config.compiler_options.jsx_import_source {
      options.transform_options.jsx.import_source = Some(jsx_import_source.clone());
    }

    if let Some(ref experimental_decorator) = ts_config.compiler_options.experimental_decorators {
      options.transform_options.decorator.legacy = *experimental_decorator;
    }

    // FIXME:
    // if user set `transform.typescript.only_remove_type_imports` to false in `rolldown.config.js`, but also set `verbatim_module_syntax` to true in `tsconfig.json`
    // We will override the value either, but actually `rolldown.config.js` should have higher priority.
    // This due to the type of `only_remove_type_imports` is `bool` we don't know if the `false` is set
    // by user or by default value.
    if let Some(ref verbatim_module_syntax) = ts_config.compiler_options.verbatim_module_syntax {
      options.transform_options.typescript.only_remove_type_imports = *verbatim_module_syntax;
    }

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
