use std::{any::Any, sync::Arc};

use rolldown_common::{BundlerOptions, FileEmitter, NormalizedBundlerOptions, SharedFileEmitter};
use rolldown_error::{BuildDiagnostic, BuildResult, EventKindSwitcher};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::{__inner::SharedPluginable, PluginDriver, SharedPluginDriver};
use rustc_hash::FxHashMap;

use crate::{
  Build,
  types::scan_stage_cache::ScanStageCache,
  utils::{
    apply_inner_plugins::apply_inner_plugins,
    prepare_build_context::{PrepareBuildContext, prepare_build_context},
  },
};

use super::super::{SharedOptions, SharedResolver};

#[derive(Debug, Default)]
pub struct BuildFactoryOptions {
  pub bundler_options: BundlerOptions,
  pub plugins: Vec<SharedPluginable>,
  pub session: Option<rolldown_debug::Session>,
  pub disable_tracing_setup: bool,
}

pub struct BuildFactory {
  pub fs: OsFileSystem,
  pub options: SharedOptions,
  pub resolver: SharedResolver,
  pub file_emitter: SharedFileEmitter,
  pub plugin_driver: SharedPluginDriver,
  /// Warnings collected during build factory creation
  pub session: rolldown_debug::Session,
  pub(crate) _log_guard: Option<Box<dyn Any + Send>>,
}

impl BuildFactory {
  pub fn new(mut opts: BuildFactoryOptions) -> BuildResult<(Self, Vec<BuildDiagnostic>)> {
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

    Ok((
      Self {
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
        _log_guard: maybe_guard,
        session,
      },
      warnings,
    ))
  }

  pub fn create_build(&mut self) -> Build {
    Build {
      fs: self.fs.clone(),
      options: Arc::clone(&self.options),
      resolver: Arc::clone(&self.resolver),
      file_emitter: Arc::clone(&self.file_emitter),
      plugin_driver: Arc::clone(&self.plugin_driver),
      warnings: Vec::new(),
      session: self.session.clone(),
      cache: ScanStageCache::default(),
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

// impl BundlerBuilder {

//   #[must_use]
//   pub fn with_options(mut self, options: BundlerOptions) -> Self {
//     self.options = options;
//     self
//   }

//   #[must_use]
//   pub fn with_plugins(mut self, plugins: Vec<SharedPluginable>) -> Self {
//     self.plugins = plugins;
//     self
//   }

//   #[must_use]
//   pub fn with_build_count(mut self, build_count: u32) -> Self {
//     self.build_count = build_count;
//     self
//   }

//   #[must_use]
//   pub fn with_session(mut self, session: rolldown_debug::Session) -> Self {
//     self.session = Some(session);
//     self
//   }

//   #[must_use]
//   pub fn with_disable_tracing_setup(mut self, disable: bool) -> Self {
//     self.disable_tracing_setup = disable;
//     self
//   }
// }
