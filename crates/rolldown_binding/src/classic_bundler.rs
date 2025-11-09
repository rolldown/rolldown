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
use rolldown_error::BuildResult;
use rolldown_plugin::__inner::SharedPluginable;
use std::sync::Arc;

pub struct ClassicBundler {
  session_id: Arc<str>,
  debug_tracer: Option<rolldown_debug::DebugTracer>,
  session: rolldown_debug::Session,
  closed: bool,
  last_bundle_handle: Option<BundleHandle>,
}

impl ClassicBundler {
  pub fn new() -> Self {
    let session_id = rolldown_debug::generate_session_id();
    Self {
      session_id,
      debug_tracer: None,
      session: rolldown_debug::Session::dummy(),
      closed: false,
      last_bundle_handle: None,
    }
  }

  pub fn create_bundle(
    &mut self,
    bundler_options: BundlerOptions,
    plugins: Vec<SharedPluginable>,
  ) -> BuildResult<Bundle> {
    if self.closed {
      return Err(rolldown_error::BuildDiagnostic::already_closed().into());
    }
    self.enable_debug_tracing_if_needed(&bundler_options);

    let mut bundle_factory = BundleFactory::new(BundleFactoryOptions {
      bundler_options,
      plugins,
      session: Some(self.session.clone()),
      disable_tracing_setup: true,
    })?;

    let bundle = bundle_factory.create_bundle(BundleMode::FullBuild, None)?;

    self.last_bundle_handle = Some(bundle.context());

    Ok(bundle)
  }

  #[must_use = "Future must be awaited to do the actual cleanup work"]
  pub fn close(&mut self) -> impl Future<Output = anyhow::Result<()>> + Send + 'static {
    let is_closed = self.closed;
    let last_bundle_handle = self.last_bundle_handle.clone();
    if !is_closed {
      self.closed = true;
    }
    // - The code is written in a non-intuitive way to satisfy the rustc and the upper usage of `BindingBundler#close`.
    // - We need the future to be `Send + 'static` for napi-rs, so we can't use `async fn` directly here.
    // - Read `BindingBundler#close` in `crates/rolldown_binding/src/binding_bundler.rs` for more details.
    async move {
      if let Some(handle) = last_bundle_handle {
        let plugin_driver = handle.plugin_driver();
        plugin_driver.close_bundle().await?;
      }
      Ok(())
    }
  }

  pub fn closed(&self) -> bool {
    self.closed
  }

  fn enable_debug_tracing_if_needed(&mut self, options: &BundlerOptions) {
    if self.debug_tracer.is_none() && options.debug.is_some() {
      self.debug_tracer = Some(rolldown_debug::DebugTracer::init(Arc::clone(&self.session_id)));
      // Caveat: `Span` must be created after initialization of `DebugTracer`, we need it to inject data to the tracking system.
      let session_span =
        tracing::debug_span!("session", CONTEXT_session_id = self.session_id.as_ref());
      // Update the `session` with the actual session span
      self.session = rolldown_debug::Session::new(Arc::clone(&self.session_id), session_span);
    }
  }
}
