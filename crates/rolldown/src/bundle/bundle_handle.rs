use std::{
  panic::AssertUnwindSafe,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use futures::FutureExt;
use rolldown_common::SharedNormalizedBundlerOptions;
use rolldown_plugin::SharedPluginDriver;
use rolldown_std_utils::{discard_panic_payload, panic_payload_message};

/// A lightweight handle to access bundle state after the `Bundle` has been consumed.
///
/// # Purpose
///
/// `BundleHandle` provides access to bundle configuration and state after the `Bundle` instance
/// has been consumed by operations like `write()`, `generate()`, or `scan()`. Since these methods
/// take ownership of the `Bundle` to prevent reuse, this handle enables:
///
/// - **Post-build cleanup**: Calling plugin lifecycle hooks like `close_bundle()` after the build completes
/// - **Watch file inspection**: Accessing the list of files that should trigger rebuilds in watch mode
/// - **Configuration access**: Reading bundler options used during the build
///
/// # Why This Exists
///
/// Rolldown's `Bundle` methods intentionally take ownership (`self`) to enforce single-use semantics
/// and prevent accidental reuse of consumed bundles. However, some operations need to access bundle
/// data after the build completes:
///
/// - `ClassicBundler` and `BundleFactory` store the last `BundleHandle` to call cleanup hooks
/// - The binding layer uses it to expose watch files to JavaScript via `get_watch_files()`
///
/// Without `BundleHandle`, these post-consumption operations would be impossible since the `Bundle`
/// has been moved and consumed.
///
/// # Usage Pattern
///
/// ```rust,ignore
/// let bundle = bundle_factory.create_bundle();
/// let handle = bundle.context(); // Extract handle before consuming
/// let output = bundle.write().await?; // Bundle consumed here
/// // Can still access data via handle:
/// let watch_files = handle.watch_files();
/// handle.plugin_driver().close_bundle().await?;
/// ```
#[derive(Clone)]
pub struct BundleHandle {
  pub(crate) options: SharedNormalizedBundlerOptions,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) closed: Arc<AtomicBool>,
}

impl BundleHandle {
  /// Get the bundler options used in this bundle.
  pub fn options(&self) -> &SharedNormalizedBundlerOptions {
    &self.options
  }

  /// Get the watch files collected during this bundle.
  ///
  /// These files should trigger a rebuild in watch mode when modified.
  pub fn watch_files(&self) -> &Arc<rolldown_utils::dashmap::FxDashSet<arcstr::ArcStr>> {
    &self.plugin_driver.watch_files
  }

  /// Get the plugin driver used in this bundle.
  ///
  /// Primarily used to call cleanup hooks like `close_bundle()` after the build completes.
  pub fn plugin_driver(&self) -> &SharedPluginDriver {
    &self.plugin_driver
  }

  /// Close this bundle handle, calling the `closeBundle` plugin hook.
  ///
  /// A panicking hook becomes an error, and resources are cleared on every outcome.
  pub async fn close(&self) -> anyhow::Result<()> {
    if self.closed.swap(true, Ordering::SeqCst) {
      return Ok(());
    }
    let result = match AssertUnwindSafe(self.plugin_driver.close_bundle(None)).catch_unwind().await
    {
      Ok(result) => result,
      Err(payload) => {
        let message = panic_payload_message(&*payload);
        // A hostile payload destructor can panic again; the shared helper
        // leaks only that nested payload, whose destructor is likewise untrusted.
        discard_panic_payload(payload);
        Err(anyhow::anyhow!("closeBundle hook panicked: {message}"))
      }
    };
    self.plugin_driver.clear();
    result
  }
}

#[cfg(test)]
mod tests {
  use crate::{BundleFactory, BundleFactoryOptions};
  use rolldown_common::BundleMode;
  use rolldown_plugin::{
    HookCloseBundleArgs, HookNoopReturn, HookUsage, Plugin, PluginContext, Pluginable,
  };
  use std::{
    borrow::Cow,
    sync::{
      Arc,
      atomic::{AtomicUsize, Ordering},
    },
  };

  #[derive(Debug)]
  struct PanickingClosePlugin {
    calls: Arc<AtomicUsize>,
  }

  #[derive(Debug)]
  struct HostilePanicPayload {
    drops: Arc<AtomicUsize>,
  }

  impl Drop for HostilePanicPayload {
    fn drop(&mut self) {
      self.drops.fetch_add(1, Ordering::SeqCst);
      panic!("close panic payload destructor escaped");
    }
  }

  #[derive(Debug)]
  struct HostilePanickingClosePlugin {
    calls: Arc<AtomicUsize>,
    payload_drops: Arc<AtomicUsize>,
  }

  impl Plugin for HostilePanickingClosePlugin {
    fn name(&self) -> Cow<'static, str> {
      "hostile-panicking-close".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
      HookUsage::CloseBundle
    }

    async fn close_bundle(
      &self,
      _ctx: &PluginContext,
      _args: Option<&HookCloseBundleArgs<'_>>,
    ) -> HookNoopReturn {
      self.calls.fetch_add(1, Ordering::SeqCst);
      std::panic::panic_any(HostilePanicPayload { drops: Arc::clone(&self.payload_drops) });
    }
  }

  impl Plugin for PanickingClosePlugin {
    fn name(&self) -> Cow<'static, str> {
      "panicking-close".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
      HookUsage::CloseBundle
    }

    async fn close_bundle(
      &self,
      _ctx: &PluginContext,
      _args: Option<&HookCloseBundleArgs<'_>>,
    ) -> HookNoopReturn {
      self.calls.fetch_add(1, Ordering::SeqCst);
      panic!("native close panic");
    }
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn close_contains_panics_and_clears_resources() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      plugins: vec![Pluginable::new_shared(PanickingClosePlugin { calls: Arc::clone(&calls) })],
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    let bundle = factory.create_bundle(BundleMode::FullBuild, None).expect("create bundle");
    let handle = bundle.context();
    handle.watch_files().insert("retained.js".into());

    let error = handle.close().await.expect_err("panicking close must become an error");
    assert!(error.to_string().contains("closeBundle hook panicked: native close panic"));
    assert!(handle.watch_files().is_empty(), "cleanup must run after a hook panic");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    handle.close().await.expect("a second close is a no-op");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn close_contains_a_panicking_payload_drop_and_clears_resources() {
    let calls = Arc::new(AtomicUsize::new(0));
    let payload_drops = Arc::new(AtomicUsize::new(0));
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      plugins: vec![Pluginable::new_shared(HostilePanickingClosePlugin {
        calls: Arc::clone(&calls),
        payload_drops: Arc::clone(&payload_drops),
      })],
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    let bundle = factory.create_bundle(BundleMode::FullBuild, None).expect("create bundle");
    let handle = bundle.context();
    handle.watch_files().insert("retained.js".into());

    let error = handle.close().await.expect_err("panicking close must become an error");
    assert_eq!(error.to_string(), "closeBundle hook panicked: non-string panic payload");
    assert!(handle.watch_files().is_empty(), "cleanup must run after payload destruction panics");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(payload_drops.load(Ordering::SeqCst), 1);

    handle.close().await.expect("a second close is a no-op");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
  }
}
