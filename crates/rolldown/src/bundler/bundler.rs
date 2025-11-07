use crate::{BuildFactory, BundlerOptions, types::scan_stage_cache::ScanStageCache};
use anyhow::Result;
use rolldown_error::BuildResult;
use rolldown_plugin::__inner::SharedPluginable;
use std::ops::Deref;

// TODO: hyf0 This's for avoiding having too many changes for watcher API. Will remove this later.
impl Deref for Bundler {
  type Target = BuildFactory;

  fn deref(&self) -> &Self::Target {
    &self.build_factory
  }
}

pub struct Bundler {
  pub(super) session: rolldown_debug::Session,
  pub(super) build_factory: BuildFactory,
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
    let build_factory = BuildFactory::new(crate::BuildFactoryOptions {
      bundler_options: options,
      plugins,
      session: None,
      disable_tracing_setup: true,
    })?;
    Ok(Self {
      build_factory,
      closed: false,
      session: rolldown_debug::Session::dummy(),
      cache: ScanStageCache::default(),
    })
  }

  pub fn with_builder_options(
    options: BundlerOptions,
    plugins: Vec<SharedPluginable>,
    session: Option<rolldown_debug::Session>,
    disable_tracing_setup: bool,
  ) -> BuildResult<Self> {
    let build_factory = BuildFactory::new(crate::BuildFactoryOptions {
      bundler_options: options,
      plugins,
      session: session.clone(),
      disable_tracing_setup,
    })?;
    Ok(Self {
      build_factory,
      closed: false,
      session: session.unwrap_or_else(rolldown_debug::Session::dummy),
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
  pub(crate) fn reset_closed_for_watch_mode(&mut self) {
    self.closed = false;
  }

  pub(super) async fn inner_close(&mut self) -> Result<()> {
    if self.closed {
      return Ok(());
    }

    self.closed = true;
    self.build_factory.plugin_driver.close_bundle().await?;

    // Clean up resources
    self.build_factory.plugin_driver.clear();
    self.cache = ScanStageCache::default();
    self.build_factory.resolver.clear_cache();
    Ok(())
  }
}

pub struct CacheGuard<'a> {
  pub is_incremental_build_enabled: bool,
  pub cache: &'a mut ScanStageCache,
}
impl CacheGuard<'_> {
  pub fn inner(&mut self) -> &mut ScanStageCache {
    self.cache
  }
}

impl Drop for CacheGuard<'_> {
  fn drop(&mut self) {
    if !self.is_incremental_build_enabled {
      std::mem::take(self.cache);
    }
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
