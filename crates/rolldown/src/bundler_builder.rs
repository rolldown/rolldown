use std::sync::Arc;

use rolldown_common::{FileEmitter, NormalizedBundlerOptions};
use rolldown_error::BuildDiagnostic;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::{__inner::SharedPluginable, PluginDriver};
use rolldown_resolver::{ResolveError, Resolver};

use crate::{
  Bundler, BundlerOptions, SharedResolver,
  types::scan_stage_cache::ScanStageCache,
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
    let mut session_id: Arc<str> = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .expect("Time went backwards")
      .as_millis()
      .to_string()
      .into();

    let mut debug_tracer = None;
    if let Some(debug_options) = &self.options.debug {
      if let Some(id) = &debug_options.session_id {
        session_id = id.to_string().into();
      }
      debug_tracer = Some(rolldown_debug::DebugTracer::init(Arc::clone(&session_id)));
    }
    let session_span = tracing::trace_span!("Session", session_id = &*session_id);

    let maybe_guard = rolldown_tracing::try_init_tracing();

    let NormalizeOptionsReturn { mut options, resolve_options, mut warnings } =
      normalize_options(self.options);
    let tsconfig_filename = resolve_options.tsconfig_filename.clone();
    let resolver: SharedResolver =
      Resolver::new(resolve_options, options.platform, options.cwd.clone(), OsFileSystem).into();

    // TODO: error handling
    Self::merge_transform_config_from_ts_config(
      &mut options,
      tsconfig_filename,
      &resolver,
      &mut warnings,
    )
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
      cache: ScanStageCache::default(),
      hmr_manager: None,
      session_span,
      _debug_tracer: debug_tracer,
    }
  }

  fn merge_transform_config_from_ts_config(
    options: &mut NormalizedBundlerOptions,
    tsconfig_filename: Option<String>,
    resolver: &SharedResolver,
    warning: &mut Vec<BuildDiagnostic>,
  ) -> Result<(), ResolveError> {
    let Some(tsconfig_filename) = tsconfig_filename else {
      return Ok(());
    };
    let ts_config = resolver.resolve_tsconfig(&options.cwd.join(&tsconfig_filename))?;
    if let Some(ref jsx_factory) = ts_config.compiler_options.jsx_factory {
      if options.transform_options.jsx.pragma.is_none() {
        options.transform_options.jsx.pragma = Some(jsx_factory.clone());
      } else {
        warning.push(
          BuildDiagnostic::configuration_field_conflict(
            "rolldown.config.js",
            "jsx.factory",
            &tsconfig_filename,
            "compilerOptions.jsxFactory",
          )
          .with_severity_warning(),
        );
      }
    }

    if let Some(ref jsx_fragment_factory) = ts_config.compiler_options.jsx_fragment_factory {
      if options.transform_options.jsx.pragma_frag.is_none() {
        options.transform_options.jsx.pragma_frag = Some(jsx_fragment_factory.clone());
      } else {
        warning.push(
          BuildDiagnostic::configuration_field_conflict(
            "rolldown.config.js",
            "jsx.fragment",
            &tsconfig_filename,
            "compilerOptions.jsxFragmentFactory",
          )
          .with_severity_warning(),
        );
      }
    }

    if let Some(ref jsx_import_source) = ts_config.compiler_options.jsx_import_source {
      if options.transform_options.jsx.import_source.is_none() {
        options.transform_options.jsx.import_source = Some(jsx_import_source.clone());
      } else {
        warning.push(
          BuildDiagnostic::configuration_field_conflict(
            "rolldown.config.js",
            "jsx.jsxImportSource",
            &tsconfig_filename,
            "compilerOptions.jsxImportSource",
          )
          .with_severity_warning(),
        );
      }
    }

    if let Some(ref experimental_decorator) = ts_config.compiler_options.experimental_decorators {
      options.transform_options.decorator.legacy = *experimental_decorator;
    }

    if let Some(ref experimental_decorator) = ts_config.compiler_options.emit_decorator_metadata {
      options.transform_options.decorator.emit_decorator_metadata = *experimental_decorator;
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
