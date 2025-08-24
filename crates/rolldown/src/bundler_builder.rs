use std::sync::Arc;

use rolldown_common::{FileEmitter, NormalizedBundlerOptions};
use rolldown_error::{BuildDiagnostic, EventKindSwitcher};
use rolldown_plugin::{__inner::SharedPluginable, PluginDriver};
use rustc_hash::FxHashMap;

use crate::{
  Bundler, BundlerOptions,
  types::scan_stage_cache::ScanStageCache,
  utils::{
    apply_inner_plugins::apply_inner_plugins,
    prepare_build_context::{PrepareBuildContext, prepare_build_context},
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

    let PrepareBuildContext { fs, resolver, options, mut warnings } =
      prepare_build_context(self.options);

    Self::check_prefer_builtin_feature(self.plugins.as_slice(), &options, &mut warnings);

    apply_inner_plugins(&options, &mut self.plugins);

    let file_emitter = Arc::new(FileEmitter::new(Arc::clone(&options)));

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
