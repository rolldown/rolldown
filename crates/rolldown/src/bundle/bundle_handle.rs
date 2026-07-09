use std::{
  any::Any,
  fmt,
  panic::{AssertUnwindSafe, catch_unwind},
  sync::{Arc, Mutex},
};

use futures::{FutureExt, future::BoxFuture, future::Shared};
use rolldown_common::SharedNormalizedBundlerOptions;
use rolldown_error::BuildDiagnostic;
use rolldown_plugin::{HookCloseBundleArgs, SharedPluginDriver};

type CloseResult = Result<(), Arc<anyhow::Error>>;
type CloseFuture = Shared<BoxFuture<'static, CloseResult>>;

#[derive(Default)]
struct BundleCloseInner {
  close_future: Option<CloseFuture>,
  close_on_error: bool,
  pending_errors: Option<Arc<Vec<BuildDiagnostic>>>,
}

#[derive(Default)]
pub(crate) struct BundleCloseState {
  inner: Mutex<BundleCloseInner>,
  resources_cleared: Mutex<bool>,
}

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

fn discard_panic_payload(payload: Box<dyn Any + Send>) {
  // A hostile payload destructor can panic again; leak only that nested payload,
  // whose destructor is likewise untrusted.
  if let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
    std::mem::forget(nested_payload);
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
  pub(crate) close_state: Arc<BundleCloseState>,
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

  #[doc(hidden)]
  pub fn close_identity(&self) -> u64 {
    self.plugin_driver.close_identity()
  }

  pub(crate) fn mark_close_on_error(&self) {
    let mut state =
      self.close_state.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.close_future.is_none() {
      state.close_on_error = true;
    }
  }

  #[doc(hidden)]
  pub fn should_close_on_error(&self) -> bool {
    self.close_state.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner).close_on_error
  }

  #[doc(hidden)]
  pub fn prepare_close_with_errors(&self, errors: Arc<Vec<BuildDiagnostic>>) {
    let mut state =
      self.close_state.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.close_future.is_none() && state.pending_errors.is_none() {
      state.pending_errors = Some(errors);
    }
  }

  /// Close this bundle handle, calling the `closeBundle` plugin hook.
  pub async fn close(&self) -> anyhow::Result<()> {
    let result = self.close_with_errors(None).await;
    let mut resources_cleared =
      self.close_state.resources_cleared.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !*resources_cleared {
      self.plugin_driver.clear();
      *resources_cleared = true;
    }
    result
  }

  pub async fn close_with_errors(
    &self,
    errors: Option<Arc<Vec<BuildDiagnostic>>>,
  ) -> anyhow::Result<()> {
    let close_future = {
      let mut state =
        self.close_state.inner.lock().expect("BundleHandle close state lock poisoned");
      if state.close_future.is_none()
        && state.pending_errors.is_none()
        && let Some(errors) = errors
      {
        state.pending_errors = Some(errors);
      }
      let errors = state.pending_errors.take();
      state.close_on_error = false;
      state
        .close_future
        .get_or_insert_with(|| {
          let plugin_driver = Arc::clone(&self.plugin_driver);
          let options = Arc::clone(&self.options);
          async move {
            let result = match AssertUnwindSafe(async {
              let args = errors
                .as_ref()
                .map(|errors| HookCloseBundleArgs { errors: errors.as_ref(), cwd: &options.cwd });
              plugin_driver.close_bundle(args.as_ref()).await
            })
            .catch_unwind()
            .await
            {
              Ok(result) => result.map_err(Arc::new),
              Err(payload) => {
                let message = panic_payload_message(&*payload).to_owned();
                discard_panic_payload(payload);
                Err(Arc::new(anyhow::anyhow!("closeBundle hook panicked: {message}")))
              }
            };
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
  use rolldown_common::{BundleMode, EmittedPrebuiltChunk};
  use rolldown_plugin::{
    HookBuildStartArgs, HookCloseBundleArgs, HookNoopReturn, HookUsage, Plugin, PluginContext,
  };
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

  #[derive(Debug)]
  struct FailingBuildStartRecordingClosePlugin {
    close_calls: Arc<AtomicUsize>,
    close_error_counts: Arc<Mutex<Vec<usize>>>,
  }

  impl Plugin for FailingBuildStartRecordingClosePlugin {
    fn name(&self) -> Cow<'static, str> {
      "failing-build-start-recording-close".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
      HookUsage::BuildStart | HookUsage::CloseBundle
    }

    async fn build_start(
      &self,
      _ctx: &PluginContext,
      _args: &HookBuildStartArgs<'_>,
    ) -> HookNoopReturn {
      Err(anyhow::anyhow!("injected buildStart failure"))
    }

    async fn close_bundle(
      &self,
      _ctx: &PluginContext,
      args: Option<&HookCloseBundleArgs<'_>>,
    ) -> HookNoopReturn {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
      self
        .close_error_counts
        .lock()
        .expect("close error counts lock poisoned")
        .push(args.map_or(0, |args| args.errors.len()));
      Ok(())
    }
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

  #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
  async fn concurrent_close_waits_for_serialized_resource_clear_completion() {
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    let bundle = factory.create_bundle(BundleMode::FullBuild, None).expect("create bundle");
    let handle = bundle.context();
    handle.watch_files().insert("retained.js".into());
    let clear_guard =
      handle.close_state.resources_cleared.lock().expect("resource clear state lock poisoned");

    let first_handle = handle.clone();
    let first = tokio::spawn(async move { first_handle.close().await });
    let second_handle = handle.clone();
    let second = tokio::spawn(async move { second_handle.close().await });

    let close_hook = loop {
      if let Some(close_hook) = handle
        .close_state
        .inner
        .lock()
        .expect("close future lock poisoned")
        .close_future
        .as_ref()
        .cloned()
      {
        break close_hook;
      }
      tokio::task::yield_now().await;
    };
    close_hook.await.expect("closeBundle should complete");
    tokio::task::yield_now().await;

    assert!(!first.is_finished(), "first close must wait for resource clearing");
    assert!(!second.is_finished(), "concurrent close must wait for resource clearing");
    assert!(handle.watch_files().contains("retained.js"));

    drop(clear_guard);
    timeout(LIVENESS_TIMEOUT, async {
      first.await.expect("first close task").expect("first close");
      second.await.expect("second close task").expect("second close");
    })
    .await
    .expect("all close callers must observe resource clearing completion");
    assert!(handle.watch_files().is_empty());
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

  #[tokio::test(flavor = "multi_thread")]
  async fn close_contains_panicking_payload_drop_clears_resources_and_replays_the_failure() {
    let calls = Arc::new(AtomicUsize::new(0));
    let payload_drops = Arc::new(AtomicUsize::new(0));
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      plugins: vec![Arc::new(HostilePanickingClosePlugin {
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

    let first_error = timeout(LIVENESS_TIMEOUT, handle.close())
      .await
      .expect("panicking payload destruction must not strand close")
      .expect_err("panicking close must become an error");
    assert_eq!(first_error.to_string(), "closeBundle hook panicked: non-string panic payload");
    assert!(handle.watch_files().is_empty(), "cleanup must run after payload destruction panics");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(payload_drops.load(Ordering::SeqCst), 1);

    let late_error = timeout(LIVENESS_TIMEOUT, handle.close())
      .await
      .expect("late close must replay the terminal result")
      .expect_err("late close must replay the panic failure");
    assert_eq!(late_error.to_string(), first_error.to_string());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(payload_drops.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn failed_scan_closes_once_with_diagnostics_and_late_close_replays_completion() {
    let close_calls = Arc::new(AtomicUsize::new(0));
    let close_error_counts = Arc::new(Mutex::new(Vec::new()));
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      plugins: vec![Arc::new(FailingBuildStartRecordingClosePlugin {
        close_calls: Arc::clone(&close_calls),
        close_error_counts: Arc::clone(&close_error_counts),
      })],
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    let bundle = factory.create_bundle(BundleMode::FullBuild, None).expect("create bundle");
    let handle = bundle.context();
    handle.watch_files().insert("retained.js".into());

    let errors = bundle.scan().await.expect_err("scan should fail in buildStart");
    assert_eq!(errors.len(), 1);
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
      close_error_counts.lock().expect("close error counts lock poisoned").as_slice(),
      [1]
    );
    assert!(
      handle.watch_files().contains("retained.js"),
      "failed builds must retain watch files until the owning close"
    );

    handle.close().await.expect("late close should replay successful completion");
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    assert!(handle.watch_files().is_empty());
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn closing_retained_full_build_does_not_clear_a_newer_full_build() {
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    let first = factory.create_bundle(BundleMode::FullBuild, None).expect("create first bundle");
    let first_handle = first.context();
    let second = factory.create_bundle(BundleMode::FullBuild, None).expect("create second bundle");
    let second_handle = second.context();

    assert!(
      !Arc::ptr_eq(
        &first_handle.plugin_driver().file_emitter,
        &second_handle.plugin_driver().file_emitter
      ),
      "independent full builds must not share clearable file-emitter state"
    );
    let reference_id =
      second_handle.plugin_driver().file_emitter.emit_prebuilt_chunk(EmittedPrebuiltChunk {
        file_name: "newer-build.js".into(),
        name: None,
        code: String::new(),
        exports: Vec::new(),
        map: None,
        sourcemap_filename: None,
        facade_module_id: None,
        is_entry: false,
        is_dynamic_entry: false,
      });

    first_handle.close().await.expect("retained first build must close");
    assert_eq!(
      second_handle
        .plugin_driver()
        .file_emitter
        .get_file_name(&reference_id)
        .expect("closing an older build must not clear the newer build"),
      "newer-build.js"
    );
  }
}
