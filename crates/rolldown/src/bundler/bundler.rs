use crate::{Bundle, BundleFactory, BundlerOptions, types::scan_stage_cache::ScanStageCache};
use rolldown_common::BundleMode;
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
  pub(super) closed: bool,
  /// Whether the terminal close failure of the current `last_bundle_handle` has
  /// already been delivered to this bundler's caller. The handle replays its
  /// terminal close result forever; the bundler reports it only once.
  /// See `ensure_last_bundle_closed` in `impl_bundler_build.rs`.
  pub(super) last_close_failure_delivered: bool,
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
      defer_close_on_error: false,
    })?;

    Ok(Self {
      bundle_factory,
      session: rolldown_devtools::Session::dummy(),
      cache: ScanStageCache::default(),
      closed: false,
      last_close_failure_delivered: false,
    })
  }

  /// The `Bundler`-level way to create a bundle. Installing a fresh
  /// `last_bundle_handle` (with a fresh close state) resets the once-per-handle
  /// close-failure delivery gate. See `ensure_last_bundle_closed` in
  /// `impl_bundler_build.rs`.
  pub(super) fn create_bundle(
    &mut self,
    bundle_mode: BundleMode,
    cache: Option<ScanStageCache>,
  ) -> BuildResult<Bundle> {
    let bundle = self.bundle_factory.create_bundle(bundle_mode, cache)?;
    self.last_close_failure_delivered = false;
    Ok(bundle)
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
