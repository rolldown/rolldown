/// `ClassicBundler` is specifically designed to satisfy the Rollup API compatibility requirements for `RolldownBuild`.
///
/// # Purpose & Use Case
///
/// `ClassicBundler` exists to bridge the architectural mismatch between Rollup's API design and Rolldown's internal requirements:
/// - **Rollup's API**: Two-step process where `rollup(inputOptions)` returns a bundle, then `bundle.write(outputOptions)` uses it
/// - **Rolldown's Reality**: Requires both `InputOptions` and `OutputOptions` together to finish a build process
/// - **ClassicBundler's Solution**: Creates a fresh `BundleFactory` and `Bundle` on each `create_bundle()` call with complete options
///
/// This design makes `ClassicBundler` suitable for one-time builds that need Rollup compatibility, but unsuitable for
/// long-running processes like watch mode or dev mode that require incremental builds and HMR.
///
/// # The Rollup API Compatibility Problem
///
/// Rollup's two-step API allows creating a bundle with only input options, then calling `write(..)`/`generate(..)` multiple
/// times with different output options:
/// ```javascript
/// const bundle = await rollup({ input: 'src/index.js' });  // Step 1: Input options only
/// await bundle.write({ dir: 'dist/esm', format: 'esm' });  // Step 2: Output options
/// await bundle.write({ dir: 'dist/cjs', format: 'cjs' });  // Can call multiple times
/// ```
///
/// However, Rolldown's architecture requires both input and output options together to create a `Bundle`. To maintain
/// Rollup compatibility, `RolldownBuild` stores the input options and merges them with output options on each
/// `generate(..)`/`write(..)` call, then uses `ClassicBundler` to create a completely fresh build each time.
///
/// # Key Architectural Differences from Core `Bundler`
///
/// `ClassicBundler` and the core `Bundler` (in `crates/rolldown/src/bundler/`) serve fundamentally different purposes:
///
/// ## BundleFactory Usage
/// - **ClassicBundler**: Creates a fresh `BundleFactory` on every `create_bundle()` call, discarding it afterwards
/// - **Core Bundler**: Creates `BundleFactory` once in constructor, reuses it across all builds
///
/// ## Cache & Incremental Builds
/// - **ClassicBundler**: No cache - every build performs a full scan from scratch
/// - **Core Bundler**: Maintains `ScanStageCache` that persists module graph, resolved paths, and symbol tables between builds
///
/// ## Build Independence
/// - **ClassicBundler**: Each `create_bundle()` call is completely independent with no shared state
/// - **Core Bundler**: Builds share factory, cache, and resolver state for incremental compilation
///
/// # Why Two Bundlers Are Needed
///
/// - **ClassicBundler**: Provides Rollup API compatibility by creating fresh builds, but cannot support incremental builds or HMR
/// - **Core Bundler**: Supports incremental builds and HMR through state persistence, but cannot satisfy Rollup's two-step API pattern
///
/// Each bundler makes different architectural trade-offs optimized for its specific use case.
///
/// # Additional Architectural Benefits
///
/// Having two bundlers with the correct mental model of state separation provides a key development benefit:
///
/// With bundler-level state (factory, cache, session) properly separated from build-level state (the `Bundle` instance),
/// new features can be developed at the `Bundle` struct level and automatically work correctly for both bundlers without
/// negative side effects. This proper abstraction layer ensures that:
///
/// - Features added to `Bundle` are isolated from bundler lifecycle concerns
/// - Both `ClassicBundler` and core `Bundler` benefit from `Bundle` improvements
/// - The codebase maintains clear separation of concerns, preventing the wrong mental model that caused bugs previously
/// - Development is more maintainable as changes are made at the appropriate abstraction level
use rolldown::{Bundle, BundleFactory, BundleFactoryOptions, BundleHandle, BundlerOptions};
use rolldown_common::BundleMode;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::__inner::SharedPluginable;
use rolldown_utils::futures::{spawn_blocking, spawn_detached};
use std::{
  any::Any,
  fmt,
  panic::{AssertUnwindSafe, catch_unwind},
  path::PathBuf,
  sync::{Arc, Mutex as StdMutex},
};

use futures::{
  FutureExt,
  channel::oneshot,
  future::{BoxFuture, Shared},
  lock::Mutex as AsyncMutex,
};

type CloseResult = Result<(), Arc<ClassicBundlerCloseError>>;
type CloseFuture = Shared<BoxFuture<'static, CloseResult>>;

struct ClassicBundlerLifecycleState {
  active_operations: usize,
  operations_drained: Vec<oneshot::Sender<()>>,
  terminal_closes: usize,
  failure_closes: usize,
  failure_closes_drained: Vec<oneshot::Sender<()>>,
  failure_close_outcomes: Vec<ClassicBundlerFailureCloseOutcome>,
}

struct ClassicBundlerLifecycle {
  state: StdMutex<ClassicBundlerLifecycleState>,
  terminal_close: AsyncMutex<()>,
}

struct ClassicBundlerFailureCloseOutcome {
  close_identity: u64,
  failures: Vec<ClassicBundlerCloseFailure>,
}

impl ClassicBundlerLifecycle {
  fn new() -> Self {
    Self {
      state: StdMutex::new(ClassicBundlerLifecycleState {
        active_operations: 0,
        operations_drained: Vec::new(),
        terminal_closes: 0,
        failure_closes: 0,
        failure_closes_drained: Vec::new(),
        failure_close_outcomes: Vec::new(),
      }),
      terminal_close: AsyncMutex::new(()),
    }
  }

  fn begin_operation(
    self: &Arc<Self>,
    debug_tracer: Option<rolldown_devtools::DebugTracer>,
  ) -> Option<ClassicBundlerOperationGuard> {
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.terminal_closes != 0 {
      return None;
    }
    state.active_operations += 1;
    drop(state);
    Some(ClassicBundlerOperationGuard { lifecycle: Arc::clone(self), active: true, debug_tracer })
  }

  fn begin_terminal_close(
    self: &Arc<Self>,
    debug_tracer: Option<rolldown_devtools::DebugTracer>,
  ) -> ClassicBundlerTerminalCloseGuard {
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    state.terminal_closes += 1;
    let operations_drained = operation_waiter(&mut state);
    ClassicBundlerTerminalCloseGuard {
      lifecycle: Arc::clone(self),
      operations_drained,
      failure_triggered: false,
      _debug_tracer: debug_tracer,
    }
  }

  async fn wait_for_failure_closes(&self) {
    let failure_closes_drained = {
      let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      failure_close_waiter(&mut state)
    };
    if let Some(failure_closes_drained) = failure_closes_drained {
      let _ = failure_closes_drained.await;
    }
  }

  fn take_failure_close_outcomes(&self) -> Vec<ClassicBundlerFailureCloseOutcome> {
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    debug_assert_eq!(state.failure_closes, 0);
    std::mem::take(&mut state.failure_close_outcomes)
  }
}

pub(crate) struct ClassicBundlerOperationGuard {
  lifecycle: Arc<ClassicBundlerLifecycle>,
  active: bool,
  debug_tracer: Option<rolldown_devtools::DebugTracer>,
}

impl ClassicBundlerOperationGuard {
  pub(crate) async fn close_after_operation(
    self,
    handle: BundleHandle,
    errors: Arc<Vec<BuildDiagnostic>>,
  ) {
    if !handle.should_close_on_error() {
      return;
    }
    handle.prepare_close_with_errors(errors);
    let terminal_close = self.into_terminal_close();
    let close_identity = handle.close_identity();
    if terminal_close.operations_drained.is_none() {
      execute_failure_close(terminal_close, handle, close_identity).await;
      return;
    }
    // See internal-docs/rust-classic-bundler/implementation.md.
    // A contended failed binding promise must settle before the unrelated
    // operations drain, while this tracked task keeps admission closed and
    // publishes its terminal outcome for the final close.
    spawn_detached(execute_failure_close(terminal_close, handle, close_identity));
  }

  fn into_terminal_close(mut self) -> ClassicBundlerTerminalCloseGuard {
    let lifecycle = Arc::clone(&self.lifecycle);
    let operations_drained = {
      let mut state = lifecycle.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      debug_assert!(self.active);
      debug_assert!(state.active_operations > 0);
      state.terminal_closes += 1;
      state.failure_closes += 1;
      state.active_operations -= 1;
      self.active = false;
      if state.active_operations == 0 {
        notify_operations_drained(&mut state);
        None
      } else {
        operation_waiter(&mut state)
      }
    };
    ClassicBundlerTerminalCloseGuard {
      lifecycle,
      operations_drained,
      failure_triggered: true,
      _debug_tracer: self.debug_tracer.take(),
    }
  }
}

async fn execute_failure_close(
  mut terminal_close: ClassicBundlerTerminalCloseGuard,
  handle: BundleHandle,
  close_identity: u64,
) {
  terminal_close.wait_for_operations().await;
  let _terminal_close = terminal_close.lifecycle.terminal_close.lock().await;
  let failures = close_bundle_failures(Some(handle)).await;
  terminal_close
    .record_failure_close_outcome(ClassicBundlerFailureCloseOutcome { close_identity, failures });
}

impl Drop for ClassicBundlerOperationGuard {
  fn drop(&mut self) {
    if !self.active {
      return;
    }
    let mut state = self.lifecycle.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    debug_assert!(state.active_operations > 0);
    state.active_operations -= 1;
    if state.active_operations == 0 {
      notify_operations_drained(&mut state);
    }
  }
}

struct ClassicBundlerTerminalCloseGuard {
  lifecycle: Arc<ClassicBundlerLifecycle>,
  operations_drained: Option<oneshot::Receiver<()>>,
  failure_triggered: bool,
  _debug_tracer: Option<rolldown_devtools::DebugTracer>,
}

impl ClassicBundlerTerminalCloseGuard {
  async fn wait_for_operations(&mut self) {
    if let Some(operations_drained) = self.operations_drained.take() {
      let _ = operations_drained.await;
    }
  }

  fn record_failure_close_outcome(&self, outcome: ClassicBundlerFailureCloseOutcome) {
    debug_assert!(self.failure_triggered);
    let mut state = self.lifecycle.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    state.failure_close_outcomes.push(outcome);
  }
}

impl Drop for ClassicBundlerTerminalCloseGuard {
  fn drop(&mut self) {
    let mut state = self.lifecycle.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    debug_assert!(state.terminal_closes > 0);
    state.terminal_closes -= 1;
    if self.failure_triggered {
      debug_assert!(state.failure_closes > 0);
      state.failure_closes -= 1;
      if state.failure_closes == 0 {
        notify_failure_closes_drained(&mut state);
      }
    }
  }
}

fn operation_waiter(state: &mut ClassicBundlerLifecycleState) -> Option<oneshot::Receiver<()>> {
  if state.active_operations == 0 {
    return None;
  }
  let (sender, receiver) = oneshot::channel();
  state.operations_drained.push(sender);
  Some(receiver)
}

fn notify_operations_drained(state: &mut ClassicBundlerLifecycleState) {
  for operations_drained in std::mem::take(&mut state.operations_drained) {
    let _ = operations_drained.send(());
  }
}

fn failure_close_waiter(state: &mut ClassicBundlerLifecycleState) -> Option<oneshot::Receiver<()>> {
  if state.failure_closes == 0 {
    return None;
  }
  let (sender, receiver) = oneshot::channel();
  state.failure_closes_drained.push(sender);
  Some(receiver)
}

fn notify_failure_closes_drained(state: &mut ClassicBundlerLifecycleState) {
  for failure_closes_drained in std::mem::take(&mut state.failure_closes_drained) {
    let _ = failure_closes_drained.send(());
  }
}

#[derive(Debug)]
pub(crate) struct ClassicBundlerCloseError {
  cwd: PathBuf,
  failures: Box<[ClassicBundlerCloseFailure]>,
}

impl ClassicBundlerCloseError {
  pub(crate) fn new(cwd: PathBuf, failures: Vec<ClassicBundlerCloseFailure>) -> Self {
    Self { cwd, failures: failures.into_boxed_slice() }
  }

  pub(crate) fn cwd(&self) -> &std::path::Path {
    &self.cwd
  }

  pub(crate) fn failures(&self) -> &[ClassicBundlerCloseFailure] {
    &self.failures
  }
}

impl fmt::Display for ClassicBundlerCloseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "Classic bundler close failed:\n- {}",
      self
        .failures
        .iter()
        .map(ClassicBundlerCloseFailure::message)
        .collect::<Vec<_>>()
        .join("\n- ")
    )
  }
}

impl std::error::Error for ClassicBundlerCloseError {}

#[derive(Debug)]
pub(crate) struct ClassicBundlerCloseFailure {
  message: Arc<str>,
  source: Option<Arc<anyhow::Error>>,
}

impl ClassicBundlerCloseFailure {
  pub(crate) fn from_error(context: &str, error: anyhow::Error) -> Self {
    let message = match catch_unwind(AssertUnwindSafe(|| format!("{context}: {error:#}"))) {
      Ok(message) => Arc::from(message),
      Err(payload) => {
        let message = Arc::from(format!(
          "{context}: error formatting panicked: {}",
          panic_payload_message(&*payload)
        ));
        discard_panic_payload(payload);
        message
      }
    };
    Self { message, source: Some(Arc::new(error)) }
  }

  pub(crate) fn from_message(message: impl Into<Arc<str>>) -> Self {
    Self { message: message.into(), source: None }
  }

  pub(crate) fn message(&self) -> &str {
    &self.message
  }

  pub(crate) fn source(&self) -> Option<&anyhow::Error> {
    self.source.as_deref()
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
  if let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
    std::mem::forget(nested_payload);
  }
}

fn panic_failure(context: &str, payload: Box<dyn Any + Send>) -> ClassicBundlerCloseFailure {
  let message = format!("{context}: {}", panic_payload_message(&*payload));
  discard_panic_payload(payload);
  ClassicBundlerCloseFailure::from_message(message)
}

async fn close_bundle_failures(
  last_bundle_handle: Option<BundleHandle>,
) -> Vec<ClassicBundlerCloseFailure> {
  let Some(handle) = last_bundle_handle else {
    return Vec::new();
  };

  match AssertUnwindSafe(handle.close()).catch_unwind().await {
    Ok(Ok(())) => Vec::new(),
    Ok(Err(error)) => {
      vec![ClassicBundlerCloseFailure::from_error("closeBundle failed", error)]
    }
    Err(payload) => vec![panic_failure("closeBundle cleanup panicked", payload)],
  }
}

async fn devtools_flush_failures(
  session: rolldown_devtools::DevtoolsSessionKey,
) -> Vec<ClassicBundlerCloseFailure> {
  let result = AssertUnwindSafe(async move {
    let rx = rolldown_devtools::flush_session(session);
    spawn_blocking(move || rx.recv_timeout(std::time::Duration::from_secs(30))).await
  })
  .catch_unwind()
  .await;

  match result {
    Ok(Ok(Ok(Ok(())))) => Vec::new(),
    Ok(Ok(Ok(Err(error)))) => error
      .into_failures()
      .into_vec()
      .into_iter()
      .map(|failure| {
        ClassicBundlerCloseFailure::from_error(
          "devtools session flush failed",
          anyhow::Error::new(failure),
        )
      })
      .collect(),
    Ok(Ok(Err(std::sync::mpsc::RecvTimeoutError::Timeout))) => {
      vec![ClassicBundlerCloseFailure::from_error(
        "devtools session flush failed",
        anyhow::anyhow!(
          "devtools writer did not acknowledge session flush within 30s; \
           node_modules/.rolldown log files may be truncated"
        ),
      )]
    }
    Ok(Ok(Err(std::sync::mpsc::RecvTimeoutError::Disconnected))) => {
      vec![ClassicBundlerCloseFailure::from_error(
        "devtools session flush failed",
        anyhow::anyhow!(
          "devtools writer thread disconnected before acknowledging flush; \
           node_modules/.rolldown log files may be truncated"
        ),
      )]
    }
    Ok(Err(error)) => vec![ClassicBundlerCloseFailure::from_error(
      "devtools session flush failed",
      anyhow::anyhow!("devtools flush task failed to join: {error}"),
    )],
    Err(payload) => vec![panic_failure("devtools session flush panicked", payload)],
  }
}

pub struct ClassicBundler {
  session_id: Arc<str>,
  debug_tracer: Option<rolldown_devtools::DebugTracer>,
  session: rolldown_devtools::Session,
  closed: bool,
  close_future: Option<CloseFuture>,
  lifecycle: Arc<ClassicBundlerLifecycle>,
  last_bundle_handle: Option<BundleHandle>,
}

impl ClassicBundler {
  pub fn new() -> Self {
    let session_id = rolldown_devtools::generate_session_id();
    Self {
      session_id,
      debug_tracer: None,
      session: rolldown_devtools::Session::dummy(),
      closed: false,
      close_future: None,
      lifecycle: Arc::new(ClassicBundlerLifecycle::new()),
      last_bundle_handle: None,
    }
  }

  pub(crate) fn create_bundle(
    &mut self,
    bundler_options: BundlerOptions,
    plugins: Vec<SharedPluginable>,
  ) -> BuildResult<(Bundle, ClassicBundlerOperationGuard)> {
    if self.closed {
      return Err(rolldown_error::BuildDiagnostic::already_closed().into());
    }
    self.enable_debug_tracing_if_needed(&bundler_options)?;
    let operation = self.lifecycle.begin_operation(self.debug_tracer.clone()).ok_or_else(|| {
      BuildDiagnostic::bundler_initialize_error(
        "Cannot start a new output while closeBundle is still running for a failed output."
          .to_string(),
        Some("Wait for the failed generate, write, or scan promise to settle.".to_string()),
      )
    })?;

    let mut bundle_factory = BundleFactory::new(BundleFactoryOptions {
      bundler_options,
      plugins,
      session: Some(self.session.clone()),
      disable_tracing_setup: true,
      defer_close_on_error: true,
    })?;

    let bundle = bundle_factory.create_bundle(BundleMode::FullBuild, None)?;
    Ok((bundle, operation))
  }

  pub fn install_bundle_handle(&mut self, handle: BundleHandle) {
    // See internal-docs/rust-classic-bundler/implementation.md.
    self.last_bundle_handle = Some(handle);
  }

  #[must_use = "Future must be awaited to observe failure-close admission reopening"]
  pub(crate) fn wait_for_failure_close(&self) -> impl Future<Output = ()> + Send + 'static {
    let lifecycle = Arc::clone(&self.lifecycle);
    async move {
      lifecycle.wait_for_failure_closes().await;
    }
  }

  #[must_use = "Future must be awaited to do the actual cleanup work"]
  pub(crate) fn close(&mut self) -> impl Future<Output = CloseResult> + Send + 'static {
    if !self.closed {
      self.closed = true;
    }
    if self.close_future.is_none() {
      let mut terminal_close = self.lifecycle.begin_terminal_close(None);
      let last_bundle_handle = self.last_bundle_handle.clone();
      let cwd =
        last_bundle_handle.as_ref().map(|handle| handle.options().cwd.clone()).unwrap_or_default();
      // Keep the fallback tracer guard alive until the authoritative flush has
      // completed. Otherwise finalizing the N-API object could enqueue a
      // destructive no-ack CloseSession while closeBundle is still running.
      let debug_tracer = self.debug_tracer.take();
      let devtools_session = debug_tracer.as_ref().map(|tracer| tracer.session_key().clone());
      self.close_future = Some(
        async move {
          terminal_close.wait_for_operations().await;
          terminal_close.lifecycle.wait_for_failure_closes().await;
          let failure_close_outcomes = terminal_close.lifecycle.take_failure_close_outcomes();
          let last_close_identity = last_bundle_handle.as_ref().map(BundleHandle::close_identity);
          let last_handle_was_failure_closed = last_close_identity.is_some_and(|identity| {
            failure_close_outcomes.iter().any(|outcome| outcome.close_identity == identity)
          });
          let mut failures = failure_close_outcomes
            .into_iter()
            .flat_map(|outcome| outcome.failures)
            .collect::<Vec<_>>();
          let latest_failures = {
            let _terminal_close = terminal_close.lifecycle.terminal_close.lock().await;
            close_bundle_failures(last_bundle_handle).await
          };
          if !last_handle_was_failure_closed {
            failures.extend(latest_failures);
          }
          if let Some(session) = devtools_session {
            failures.extend(devtools_flush_failures(session).await);
          }
          let result = if failures.is_empty() {
            Ok(())
          } else {
            Err(Arc::new(ClassicBundlerCloseError::new(cwd, failures)))
          };
          // This may enqueue one harmless no-ack close after the authoritative
          // result has already drained and captured the session state.
          drop(debug_tracer);
          result
        }
        .boxed()
        .shared(),
      );
    }
    let close_future = self.close_future.as_ref().expect("close future initialized above").clone();
    // - The code is written in a non-intuitive way to satisfy the rustc and the upper usage of `BindingBundler#close`.
    // - We need the future to be `Send + 'static` for napi-rs, so we can't use `async fn` directly here.
    // - Read `BindingBundler#close` in `crates/rolldown_binding/src/binding_bundler.rs` for more details.
    close_future
  }

  pub fn closed(&self) -> bool {
    self.closed
  }

  fn enable_debug_tracing_if_needed(&mut self, options: &BundlerOptions) -> BuildResult<()> {
    if self.debug_tracer.is_some() || !self.configure_devtools_session_id(options) {
      return Ok(());
    }
    let devtools_cwd_path = options.cwd.as_deref().unwrap_or_else(|| std::path::Path::new(""));
    let debug_tracer =
      rolldown_devtools::DebugTracer::init(Arc::clone(&self.session_id), devtools_cwd_path)
        .map_err(|error| {
          BuildDiagnostic::bundler_initialize_error(
            format!("Failed to enable devtools tracing: {error}"),
            None,
          )
        })?;
    // Caveat: `Span` must be created after initialization of `DebugTracer`, we need it to inject data to the tracking system.
    let session_span = tracing::debug_span!(
      "session",
      CONTEXT_session_id = self.session_id.as_ref(),
      CONTEXT_devtools_output_root = debug_tracer.session_key().output_root()
    );
    self.debug_tracer = Some(debug_tracer);
    // Update the `session` with the actual session span
    self.session = rolldown_devtools::Session::new(Arc::clone(&self.session_id), session_span);
    Ok(())
  }

  fn configure_devtools_session_id(&mut self, options: &BundlerOptions) -> bool {
    let Some(devtools) = options.devtools.as_ref() else {
      return false;
    };
    if let Some(session_id) = devtools.session_id.as_deref() {
      self.session_id = Arc::from(session_id);
    }
    true
  }
}

#[cfg(test)]
mod tests {
  use std::{
    error::Error,
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
  };

  use futures::{future::join, pin_mut, poll};

  use super::*;

  #[derive(Debug)]
  struct PanickingDisplayError;

  impl fmt::Display for PanickingDisplayError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
      panic!("injected Display panic");
    }
  }

  impl Error for PanickingDisplayError {}

  #[test]
  fn close_failure_contains_a_panicking_error_formatter() {
    let failure = ClassicBundlerCloseFailure::from_error(
      "closeBundle failed",
      anyhow::Error::new(PanickingDisplayError),
    );

    assert_eq!(
      failure.message(),
      "closeBundle failed: error formatting panicked: injected Display panic"
    );
    assert!(
      failure.source().and_then(|error| error.downcast_ref::<PanickingDisplayError>()).is_some()
    );
  }

  #[test]
  fn devtools_session_id_uses_the_requested_value_or_generated_fallback() {
    let mut bundler = ClassicBundler::new();
    let generated_session_id = Arc::clone(&bundler.session_id);

    let disabled_options = BundlerOptions::default();
    assert!(!bundler.configure_devtools_session_id(&disabled_options));
    assert!(Arc::ptr_eq(&bundler.session_id, &generated_session_id));

    let fallback_options =
      BundlerOptions { devtools: Some(rolldown::DevtoolsOptions::default()), ..Default::default() };
    assert!(bundler.configure_devtools_session_id(&fallback_options));
    assert!(Arc::ptr_eq(&bundler.session_id, &generated_session_id));

    let requested_options = BundlerOptions {
      devtools: Some(rolldown::DevtoolsOptions {
        session_id: Some("requested-session".to_string()),
      }),
      ..Default::default()
    };
    assert!(bundler.configure_devtools_session_id(&requested_options));
    assert_eq!(bundler.session_id.as_ref(), "requested-session");
  }

  #[test]
  fn close_coalesces_and_replays_one_terminal_outcome() {
    futures::executor::block_on(async {
      let calls = Arc::new(AtomicUsize::new(0));
      let expected_error = Arc::new(ClassicBundlerCloseError::new(
        PathBuf::new(),
        vec![ClassicBundlerCloseFailure::from_message("retained close failure")],
      ));
      let future_calls = Arc::clone(&calls);
      let future_error = Arc::clone(&expected_error);
      let mut bundler = ClassicBundler::new();
      bundler.close_future = Some(
        async move {
          future_calls.fetch_add(1, Ordering::SeqCst);
          Err(future_error)
        }
        .boxed()
        .shared(),
      );

      let (first, concurrent) = join(bundler.close(), bundler.close()).await;
      let first = first.expect_err("first close must return the retained failure");
      let concurrent = concurrent.expect_err("concurrent close must return the retained failure");
      assert!(Arc::ptr_eq(&first, &expected_error));
      assert!(Arc::ptr_eq(&concurrent, &expected_error));
      assert_eq!(calls.load(Ordering::SeqCst), 1);

      let late = bundler.close().await.expect_err("late close must replay the retained failure");
      assert!(Arc::ptr_eq(&late, &expected_error));
      assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
  }

  #[test]
  fn close_waits_for_every_active_operation_before_terminal_cleanup() {
    futures::executor::block_on(async {
      let mut bundler = ClassicBundler::new();
      let first_operation = bundler.lifecycle.begin_operation(None).expect("first operation");
      let second_operation = bundler.lifecycle.begin_operation(None).expect("second operation");
      let close = bundler.close();
      pin_mut!(close);

      assert!(matches!(poll!(close.as_mut()), Poll::Pending));
      drop(first_operation);
      assert!(matches!(poll!(close.as_mut()), Poll::Pending));
      drop(second_operation);
      close.await.expect("close should finish after the operation barrier drains");
    });
  }

  #[test]
  fn failure_terminal_close_blocks_new_operations_until_the_guard_drops() {
    let lifecycle = Arc::new(ClassicBundlerLifecycle::new());
    let operation = lifecycle.begin_operation(None).expect("initial operation");
    let terminal_close = operation.into_terminal_close();

    assert!(
      lifecycle.begin_operation(None).is_none(),
      "failure close must reserve operation admission before leaving the active set"
    );

    drop(terminal_close);
    assert!(
      lifecycle.begin_operation(None).is_some(),
      "operation admission must resume after failure close finishes"
    );
  }

  #[test]
  fn operation_guard_retains_debug_tracer_after_classic_bundler_drops() {
    let session_id: Arc<str> =
      format!("classic-bundler-operation-guard-{}", std::process::id()).into();
    let tracer = rolldown_devtools::DebugTracer::init(
      Arc::clone(&session_id),
      &std::env::temp_dir().join(session_id.as_ref()),
    )
    .expect("initialize debug tracer");
    let expected_session = tracer.session_key().clone();
    let mut bundler = ClassicBundler::new();
    bundler.debug_tracer = Some(tracer);
    let operation = bundler
      .lifecycle
      .begin_operation(bundler.debug_tracer.clone())
      .expect("operation should start");

    drop(bundler);

    assert_eq!(
      operation
        .debug_tracer
        .as_ref()
        .expect("operation must retain the tracer lease")
        .session_key(),
      &expected_session
    );
  }
}
