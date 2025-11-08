use std::{any::Any, sync::Arc};

use rolldown_common::{BundlerOptions, FileEmitter, NormalizedBundlerOptions, SharedFileEmitter};
use rolldown_error::{BuildDiagnostic, BuildResult, EventKindSwitcher};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::{__inner::SharedPluginable, PluginDriver, SharedPluginDriver};
use rustc_hash::FxHashMap;

use crate::{
  Bundle,
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
  pub session: Option<rolldown_debug::Session>,
  pub disable_tracing_setup: bool,
}

pub struct BundleFactory {
  pub fs: OsFileSystem,
  pub options: SharedOptions,
  pub resolver: SharedResolver,
  pub file_emitter: SharedFileEmitter,
  pub plugin_driver: SharedPluginDriver,
  /// Warnings collected during bundle factory creation.
  /// These warnings are transferred to the first created `Bundle` via `create_bundle()` or `create_incremental_bundle()`.
  pub warnings: Vec<BuildDiagnostic>,
  pub session: rolldown_debug::Session,
  pub(crate) _log_guard: Option<Box<dyn Any + Send>>,
}

impl BundleFactory {
  pub fn new(mut opts: BundleFactoryOptions) -> BuildResult<Self> {
    let session = opts.session.unwrap_or_else(rolldown_debug::Session::dummy);

    let maybe_guard =
      if opts.disable_tracing_setup { None } else { rolldown_tracing::try_init_tracing() };

    let PrepareBuildContext { fs, resolver, options, mut warnings } =
      prepare_build_context(opts.bundler_options)?;

    Self::check_prefer_builtin_feature(opts.plugins.as_slice(), &options, &mut warnings);

    apply_inner_plugins(&options, &mut opts.plugins);

    let file_emitter = Arc::new(FileEmitter::new(Arc::clone(&options)));

    // FIXME: shouldn't crate build span here, but for satisfying the previous code
    let build_id = rolldown_debug::generate_build_id(0);
    let build_span = Arc::new(tracing::info_span!(
      parent: &session.span,
      "build",
      CONTEXT_build_id = build_id.as_ref()
    ));

    Ok(Self {
      plugin_driver: PluginDriver::new_shared(
        opts.plugins,
        &resolver,
        &file_emitter,
        &options,
        &session,
        &build_span,
      ),
      file_emitter,
      resolver,
      options,
      fs,
      warnings,
      _log_guard: maybe_guard,
      session,
    })
  }

  pub fn create_bundle(&mut self) -> Bundle {
    Bundle {
      fs: self.fs.clone(),
      options: Arc::clone(&self.options),
      resolver: Arc::clone(&self.resolver),
      file_emitter: Arc::clone(&self.file_emitter),
      plugin_driver: Arc::clone(&self.plugin_driver),
      warnings: std::mem::take(&mut self.warnings),
      session: self.session.clone(),
      cache: ScanStageCache::default(),
    }
  }

  pub fn create_incremental_bundle(&mut self, cache: ScanStageCache) -> Bundle {
    Bundle {
      fs: self.fs.clone(),
      options: Arc::clone(&self.options),
      resolver: Arc::clone(&self.resolver),
      file_emitter: Arc::clone(&self.file_emitter),
      plugin_driver: Arc::clone(&self.plugin_driver),
      warnings: std::mem::take(&mut self.warnings),
      session: self.session.clone(),
      cache,
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
