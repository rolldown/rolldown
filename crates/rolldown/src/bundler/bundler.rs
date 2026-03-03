use crate::{BundleFactory, BundlerOptions, types::scan_stage_cache::ScanStageCache};
use anyhow::Result;
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
  pub(super) cache: ScanStageCache,
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
      closed: false,
      session: rolldown_devtools::Session::dummy(),
      cache: ScanStageCache::default(),
    })
  }

  // Implementation is split across multiple files:
  // - Normal build operations and lifecycle: `impl_bundler_build.rs`
  // - Getter/accessor methods: `impl_bundler_getter.rs`
  // - Incremental build methods: `impl_bundler_incremental_build.rs`
  // - HMR methods: `impl_bundler_hmr.rs`

  pub(super) fn create_error_if_closed(&self) -> BuildResult<()> {
    if self.closed {
      Err(anyhow::anyhow!("Bundler is closed"))?;
    }
    Ok(())
  }

  // Rollup always creates a new build in watch mode, which could be called multiple times.
  // Here only reset the closed flag to make it possible to call again.
  pub fn reset_closed_for_watch_mode(&mut self) {
    self.closed = false;
  }

  pub(super) async fn inner_close(&mut self) -> Result<()> {
    if self.closed {
      return Ok(());
    }

    self.closed = true;
    // NOTE: `close_bundle` is not called if no bundle happened: https://github.com/rolldown/rolldown/issues/6910
    if let Some(last_bundle_handle) = &self.last_bundle_handle {
      last_bundle_handle.plugin_driver.close_bundle(None).await?;
      last_bundle_handle.plugin_driver.clear();
    }

    // Clean up resources
    self.cache = ScanStageCache::default();
    self.bundle_factory.resolver.clear_cache();
    Ok(())
  }
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
