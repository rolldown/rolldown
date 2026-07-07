use std::{
  any::Any,
  fmt,
  panic::AssertUnwindSafe,
  sync::{Arc, Mutex},
};

use futures::{FutureExt, future::BoxFuture, future::Shared};
use rolldown_common::SharedNormalizedBundlerOptions;
use rolldown_plugin::SharedPluginDriver;

type CloseResult = Result<(), Arc<anyhow::Error>>;
type CloseFuture = Shared<BoxFuture<'static, CloseResult>>;

#[derive(Debug)]
struct SharedCloseError(Arc<anyhow::Error>);

impl SharedCloseError {
  fn new(error: Arc<anyhow::Error>) -> Self {
    Self(error)
  }
}

impl fmt::Display for SharedCloseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:#}", self.0)
  }
}

impl std::error::Error for SharedCloseError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(self.0.root_cause())
  }
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> &str {
  if let Some(message) = payload.downcast_ref::<String>() {
    message
  } else if let Some(message) = payload.downcast_ref::<&str>() {
    message
  } else {
    "non-string panic payload"
  }
}

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
  pub(crate) close_future: Arc<Mutex<Option<CloseFuture>>>,
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
  pub async fn close(&self) -> anyhow::Result<()> {
    let close_future = {
      let mut state = self.close_future.lock().expect("BundleHandle close state lock poisoned");
      state
        .get_or_insert_with(|| {
          let plugin_driver = Arc::clone(&self.plugin_driver);
          async move {
            let result =
              match AssertUnwindSafe(plugin_driver.close_bundle(None)).catch_unwind().await {
                Ok(result) => result.map_err(Arc::new),
                Err(payload) => Err(Arc::new(anyhow::anyhow!(
                  "closeBundle hook panicked: {}",
                  panic_payload_message(&*payload)
                ))),
              };
            plugin_driver.clear();
            result
          }
          .boxed()
          .shared()
        })
        .clone()
    };

    close_future.await.map_err(|error| anyhow::Error::new(SharedCloseError::new(error)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{BundleFactory, BundleFactoryOptions};
  use rolldown_common::BundleMode;
  use rolldown_plugin::{HookCloseBundleArgs, HookNoopReturn, HookUsage, Plugin, PluginContext};
  use std::{
    borrow::Cow,
    sync::atomic::{AtomicUsize, Ordering},
  };
  use tokio::{
    sync::Notify,
    time::{Duration, timeout},
  };

  const LIVENESS_TIMEOUT: Duration = Duration::from_secs(10);

  #[derive(Debug)]
  struct GatedFailingClosePlugin {
    calls: Arc<AtomicUsize>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
  }

  #[derive(Debug)]
  struct PanickingClosePlugin {
    calls: Arc<AtomicUsize>,
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

  impl Plugin for GatedFailingClosePlugin {
    fn name(&self) -> Cow<'static, str> {
      "gated-failing-close".into()
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
      self.entered.notify_one();
      self.release.notified().await;
      Err(anyhow::anyhow!("close bundle failed"))
    }
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn close_waits_for_and_replays_one_terminal_result_while_clearing_resources() {
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      plugins: vec![Arc::new(GatedFailingClosePlugin {
        calls: Arc::clone(&calls),
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
      })],
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    let bundle = factory.create_bundle(BundleMode::FullBuild, None).expect("create bundle");
    let handle = bundle.context();
    handle.watch_files().insert("retained.js".into());

    let first_handle = handle.clone();
    let mut first = tokio::spawn(async move { first_handle.close().await });
    tokio::select! {
      () = entered.notified() => {}
      result = &mut first => {
        panic!("first close completed before entering closeBundle: {result:?}");
      }
    }

    let second_handle = handle.clone();
    let second = tokio::spawn(async move { second_handle.close().await });
    tokio::task::yield_now().await;
    assert!(!second.is_finished(), "concurrent close must wait for the hook");

    release.notify_one();
    let (first_error, second_error) = timeout(LIVENESS_TIMEOUT, async {
      let first_error =
        first.await.expect("first close task").expect_err("first close should fail");
      let second_error =
        second.await.expect("second close task").expect_err("second close should fail");
      (first_error, second_error)
    })
    .await
    .expect("all close callers must finish before the liveness deadline");
    assert!(first_error.to_string().contains("close bundle failed"));
    assert_eq!(second_error.to_string(), first_error.to_string());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(handle.watch_files().is_empty());

    let late_error = handle.close().await.expect_err("late close should replay failure");
    assert_eq!(late_error.to_string(), first_error.to_string());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn close_contains_panics_clears_resources_and_replays_the_failure() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      plugins: vec![Arc::new(PanickingClosePlugin { calls: Arc::clone(&calls) })],
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    let bundle = factory.create_bundle(BundleMode::FullBuild, None).expect("create bundle");
    let handle = bundle.context();
    handle.watch_files().insert("retained.js".into());

    let first_error = handle.close().await.expect_err("panicking close must become an error");
    assert!(first_error.to_string().contains("closeBundle hook panicked: native close panic"));
    assert!(handle.watch_files().is_empty(), "cleanup must run after a hook panic");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let late_error = handle.close().await.expect_err("late close must replay the panic failure");
    assert_eq!(late_error.to_string(), first_error.to_string());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
  }
}
