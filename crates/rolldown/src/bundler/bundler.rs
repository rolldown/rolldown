use crate::{BundleFactory, BundlerOptions, types::scan_stage_cache::ScanStageCache};
use rolldown_error::BuildResult;
use rolldown_plugin::__inner::SharedPluginable;
use std::ops::Deref;

// TODO: hyf0 This's for avoiding having too many changes for watcher API. Will remove this later.
impl Deref for Bundler {
  type Target = BundleFactory;

  fn deref(&self) -> &Self::Target {
    &self.bundle_factory
  }
}

pub struct Bundler {
  pub(super) session: rolldown_devtools::Session,
  pub(super) bundle_factory: BundleFactory,
  #[cfg_attr(not(feature = "experimental"), allow(dead_code))]
  pub(super) cache: ScanStageCache,
  /// An HMR update has three stages:
  ///
  /// 1. Refetch the changed modules.
  /// 2. Merge them into the module graph.
  /// 3. Render each client's patch.
  ///
  /// A failure in step 1 leaves the graph unchanged, so the next update retries the edit. A
  /// failure in step 2 or 3 loses the edit: no client ran it, and the graph already has it.
  /// This flag records that loss. While it is set, the dev engine reloads every client and
  /// runs a full build. A successful full build clears it. It lives outside `cache` because
  /// a full scan drops `cache`, even when the scan fails.
  pub(super) lost_hmr_update: bool,
  pub(super) closed: bool,
}

impl Bundler {
  pub fn new(options: BundlerOptions) -> BuildResult<Self> {
    Self::with_plugins(options, Vec::new())
  }

  pub fn with_plugins(
    options: BundlerOptions,
    plugins: Vec<SharedPluginable>,
  ) -> BuildResult<Self> {
    let bundle_factory = BundleFactory::new(crate::BundleFactoryOptions {
      bundler_options: options,
      plugins,
      session: None,
      disable_tracing_setup: true,
    })?;

    Ok(Self {
      bundle_factory,
      session: rolldown_devtools::Session::dummy(),
      cache: ScanStageCache::default(),
      lost_hmr_update: false,
      closed: false,
    })
  }

  pub(super) fn create_error_if_closed(&self) -> BuildResult<()> {
    if self.closed {
      Err(anyhow::anyhow!("Bundler is closed"))?;
    }
    Ok(())
  }

  // Implementation is split across multiple files:
  // - Normal build operations and lifecycle: `impl_bundler_build.rs`
  // - Getter/accessor methods: `impl_bundler_getter.rs`
  // - Incremental build methods: `impl_bundler_incremental_build.rs`
  // - HMR methods: `impl_bundler_hmr.rs`
}

fn _test_bundler() {
  fn assert_send(_foo: impl Send) {}
  let mut bundler = Bundler::new(BundlerOptions::default()).expect("Failed to create bundler");
  let write_fut = bundler.write();
  assert_send(write_fut);
  let mut bundler = Bundler::new(BundlerOptions::default()).expect("Failed to create bundler");
  let generate_fut = bundler.generate();
  assert_send(generate_fut);
}
