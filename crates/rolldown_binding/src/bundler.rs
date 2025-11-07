// TODO: add reasons about why we creating another `Bundler` instead of reusing `Bundler` of `rolldown` crate.

use rolldown::{Build, BuildContext, BuildFactory, BuildFactoryOptions, BundlerOptions};
use rolldown_error::BuildResult;
use rolldown_plugin::__inner::SharedPluginable;
use std::sync::Arc;

pub struct Bundler {
  session_id: Arc<str>,
  debug_tracer: Option<rolldown_debug::DebugTracer>,
  session: rolldown_debug::Session,
  closed: bool,
  last_build_context: Option<BuildContext>,
}

impl Bundler {
  pub fn new() -> Self {
    let session_id = rolldown_debug::generate_session_id();
    Self {
      session_id,
      debug_tracer: None,
      session: rolldown_debug::Session::dummy(),
      closed: false,
      last_build_context: None,
    }
  }

  pub fn create_build(
    &mut self,
    bundler_options: BundlerOptions,
    plugins: Vec<SharedPluginable>,
  ) -> BuildResult<(Build, Vec<rolldown_error::BuildDiagnostic>)> {
    if self.closed {
      return Err(rolldown_error::BuildDiagnostic::already_closed().into());
    }
    self.enable_debug_tracing_if_needed(&bundler_options);

    let (mut build_factory, warnings) = BuildFactory::new(BuildFactoryOptions {
      bundler_options,
      plugins,
      session: Some(self.session.clone()),
      disable_tracing_setup: true,
    })?;

    let build = build_factory.create_build();

    self.last_build_context = Some(build.context());

    Ok((build, warnings))
  }

  #[must_use = "Future must be awaited to do the actual cleanup work"]
  pub fn close(&mut self) -> impl Future<Output = anyhow::Result<()>> + Send + 'static {
    let is_closed = self.closed;
    let last_build_context = self.last_build_context.clone();
    if !is_closed {
      self.closed = true;
    }
    // - The code is written in a non-intuitive way to satisfy the rustc and the upper usage of `BindingBundler#close`.
    // - We need the future to be `Send + 'static` for napi-rs, so we can't use `async fn` directly here.
    // - Read `BindingBundler#close` in `crates/rolldown_binding/src/binding_bundler.rs` for more details.
    async move {
      if let Some(context) = last_build_context {
        let plugin_driver = context.plugin_driver();
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
