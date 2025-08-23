use std::sync::Arc;

use rolldown_common::{FileEmitter, NormalizedBundlerOptions};
use rolldown_error::{BuildDiagnostic, EventKindSwitcher};
use rolldown_fs::{OsFileSystem, OxcResolverFileSystem};
use rolldown_plugin::{__inner::SharedPluginable, PluginDriver};
use rolldown_resolver::{ResolveError, Resolver};
use rustc_hash::FxHashMap;

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
  session: Option<rolldown_debug::Session>,
  build_count: u32,
}

impl BundlerBuilder {
  pub fn build(mut self) -> Bundler {
    let session = self.session.unwrap_or_else(rolldown_debug::Session::dummy);

    let maybe_guard = rolldown_tracing::try_init_tracing();

    let NormalizeOptionsReturn { mut options, resolve_options, mut warnings } =
      normalize_options(self.options);

    Self::check_prefer_builtin_feature(self.plugins.as_slice(), &options, &mut warnings);

    let tsconfig_filename = resolve_options.tsconfig_filename.clone();
    let fs = OsFileSystem::new(resolve_options.yarn_pnp.is_some_and(|b| b));
    let resolver: SharedResolver =
      Resolver::new(resolve_options, options.platform, options.cwd.clone(), fs.clone()).into();

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

    apply_inner_plugins(&options, &mut self.plugins);

    Bundler {
      closed: false,
      plugin_driver: PluginDriver::new_shared(self.plugins, &resolver, &file_emitter, &options),
      file_emitter,
      resolver,
      options,
      fs,
      warnings,
      _log_guard: maybe_guard,
      cache: ScanStageCache::default(),
      hmr_manager: None,
      session,
      build_count: self.build_count,
      last_error: None,
    }
  }

  fn check_prefer_builtin_feature(
    plugins: &[SharedPluginable],
    options: &NormalizedBundlerOptions,
    warning: &mut Vec<BuildDiagnostic>,
  ) {
    if !options.checks.contains(EventKindSwitcher::PreferBuiltinFeature) {
      return;
    }
    let map = FxHashMap::from_iter([
      // key is the name property of the plugin
      // the first element of value is the npm package name of the plugin
      // the second element of value is the preferred builtin feature, `None` if the feature is not configured
      ("inject", ("@rollup/plugin-inject", Some("inject"))),
      ("node-resolve", (" @rollup/plugin-node-resolve", None)),
      ("commonjs", ("@rollup/plugin-commonjs", None)),
      ("json", ("@rollup/plugin-json", None)),
    ]);
    for plugin in plugins {
      let name = plugin.call_name();
      let Some((package_name, feature)) = map.get(name.as_ref()) else {
        continue;
      };
      warning.push(
        BuildDiagnostic::prefer_builtin_feature(
          feature.map(String::from),
          (*package_name).to_string(),
        )
        .with_severity_warning(),
      );
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
}
