use std::{any::Any, sync::Arc};

use arcstr::ArcStr;
use dashmap::DashMap;
use rolldown_common::{
  BundleMode, BundlerOptions, FileEmitter, ModuleIdx, NormalizedBundlerOptions, SharedFileEmitter,
  SharedModuleInfoDashMap,
};
use rolldown_error::{BuildDiagnostic, BuildResult, EventKindSwitcher};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::{__inner::SharedPluginable, PluginDriverFactory};
use rolldown_plugin_lazy_compilation::LazyCompilationContext;
use rolldown_utils::dashmap::FxDashSet;
use rustc_hash::FxHashMap;

use crate::{
  Bundle, BundleHandle,
  types::scan_stage_cache::ScanStageCache,
  utils::{
    apply_inner_plugins::apply_inner_plugins,
    prepare_build_context::{PrepareBuildContext, prepare_build_context},
  },
};

use super::super::{SharedOptions, SharedResolver};

#[derive(Debug, Default)]
pub struct BundleFactoryOptions {
  pub bundler_options: BundlerOptions,
  pub plugins: Vec<SharedPluginable>,
  pub session: Option<rolldown_devtools::Session>,
  pub disable_tracing_setup: bool,
}

pub struct BundleFactory {
  pub plugin_driver_factory: PluginDriverFactory,
  pub fs: OsFileSystem,
  pub options: SharedOptions,
  pub resolver: SharedResolver,
  pub file_emitter: SharedFileEmitter,
  /// Warnings collected during bundle factory creation.
  /// These warnings are transferred to the first created `Bundle` via `create_bundle()` or `create_incremental_bundle()`.
  pub warnings: Vec<BuildDiagnostic>,
  pub session: rolldown_devtools::Session,
  pub(crate) _log_guard: Option<Box<dyn Any + Send>>,
  pub last_bundle_handle: Option<BundleHandle>,

  // Used to share module info across multiple plugin drivers for incremental builds
  module_infos_for_incremental_build: SharedModuleInfoDashMap,

  // Used to preserve transform dependencies (from addWatchFile) across incremental builds for HMR
  transform_dependencies_for_incremental_build: Arc<DashMap<ModuleIdx, Arc<FxDashSet<ArcStr>>>>,

  // Used to generate unique id for each bundle process
  bundle_id_seed: u32,

  /// Context for lazy compilation, if enabled
  pub lazy_compilation_context: Option<LazyCompilationContext>,
}

impl BundleFactory {
  pub fn new(mut opts: BundleFactoryOptions) -> BuildResult<Self> {
    let session = opts.session.unwrap_or_else(rolldown_devtools::Session::dummy);

    let maybe_guard =
      if opts.disable_tracing_setup { None } else { rolldown_tracing::try_init_tracing() };

    let PrepareBuildContext { fs, resolver, options, mut warnings } =
      prepare_build_context(opts.bundler_options)?;

    Self::check_prefer_builtin_feature(opts.plugins.as_slice(), &options, &mut warnings);

    let inner_plugins_result = apply_inner_plugins(&options, &mut opts.plugins);

    let file_emitter = Arc::new(FileEmitter::new(Arc::clone(&options)));

    let plugin_driver_factory = PluginDriverFactory::new(opts.plugins, &resolver);

    Ok(Self {
      plugin_driver_factory,
      file_emitter,
      resolver,
      options,
      fs,
      warnings,
      _log_guard: maybe_guard,
      session,
      bundle_id_seed: 0,
      last_bundle_handle: None,
      module_infos_for_incremental_build: Arc::default(),
      transform_dependencies_for_incremental_build: Arc::default(),
      lazy_compilation_context: inner_plugins_result.lazy_compilation_context,
    })
  }

  fn generate_unique_bundle_span(&mut self) -> Arc<tracing::Span> {
    let bundle_id = rolldown_devtools::generate_build_id(self.bundle_id_seed);
    self.bundle_id_seed += 1;
    Arc::new(tracing::info_span!(
      parent: &self.session.span,
      "build",
      CONTEXT_build_id = bundle_id.as_ref(),
      // - This behaves like default value for `${hook_resolve_id_trigger}`.
      // - For case like injecting `manual`, we will override this field by adding a child span to shadow this one.
      CONTEXT_hook_resolve_id_trigger = "automatic"
    ))
  }

  pub fn create_bundle(
    &mut self,
    bundle_mode: BundleMode,
    cache: Option<ScanStageCache>,
  ) -> BuildResult<Bundle> {
    let bundle_span = self.generate_unique_bundle_span();

    let cache = if bundle_mode.is_incremental() {
      if let Some(cache) = cache {
        cache
      } else {
        Err(anyhow::anyhow!(
          "Incremental bundle requires a valid ScanStageCache, but none was provided."
        ))?
      }
    } else {
      // Use a default cache as placeholder for full build
      ScanStageCache::default()
    };

    if bundle_mode.is_full_build() {
      // Reset module infos for full bundle and store it for potential incremental builds
      self.module_infos_for_incremental_build = Arc::default();
      // Also reset transform dependencies for full builds
      self.transform_dependencies_for_incremental_build = Arc::default();
    }
    let module_infos = Arc::clone(&self.module_infos_for_incremental_build);
    let transform_dependencies = Arc::clone(&self.transform_dependencies_for_incremental_build);

    let plugin_driver = self.plugin_driver_factory.create_plugin_driver(
      &self.file_emitter,
      &self.options,
      &self.session,
      &bundle_span,
      module_infos,
      transform_dependencies,
    );
    let bundle = Bundle {
      fs: self.fs.clone(),
      options: Arc::clone(&self.options),
      resolver: Arc::clone(&self.resolver),
      file_emitter: Arc::clone(&self.file_emitter),
      plugin_driver,
      warnings: std::mem::take(&mut self.warnings),
      bundle_span,
      cache,
    };
    self.last_bundle_handle = Some(bundle.context());
    Ok(bundle)
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
      // the third element of value is an additional message to show
      ("inject", ("@rollup/plugin-inject", Some("inject"), None)),
      ("node-resolve", ("@rollup/plugin-node-resolve", None, None)),
      (
        "commonjs",
        (
          "@rollup/plugin-commonjs",
          None,
          Some(" Check https://rolldown.rs/in-depth/bundling-cjs for more details."),
        ),
      ),
      ("json", ("@rollup/plugin-json", None, None)),
    ]);
    for plugin in plugins {
      let name = plugin.call_name();
      let Some((package_name, feature, additional_message)) = map.get(name.as_ref()) else {
        continue;
      };
      warning.push(
        BuildDiagnostic::prefer_builtin_feature(
          feature.map(String::from),
          (*package_name).to_string(),
          *additional_message,
        )
        .with_severity_warning(),
      );
    }
  }
}
