use std::sync::Arc;

use anyhow::Result;
use rolldown_common::{BundlerOptions, NotifyOption};
use rolldown_error::BuildResult;
use rolldown_plugin::__inner::SharedPluginable;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  watch::watcher::{WatcherImpl, wait_for_change},
};

pub struct Watcher(Arc<WatcherImpl>);

impl Watcher {
  /// Creates a new watcher from bundler options and plugins.
  /// This performs validation and constructs bundlers internally.
  pub fn new(
    options_and_plugins: Vec<(BundlerOptions, Vec<SharedPluginable>)>,
    notify_option: Option<NotifyOption>,
  ) -> BuildResult<Self> {
    // Validate that HMR is not enabled for watcher
    for (options, _) in &options_and_plugins {
      if options.experimental.as_ref().and_then(|e| e.hmr.as_ref()).is_some() {
        return Err(rolldown_error::BuildDiagnostic::bundler_initialize_error(
          "The \"experimental.hmr\" option is only supported with the \"dev\" API. It cannot be used with \"watch\". Please use the \"dev\" API for HMR functionality.".to_string(),
          None,
        ).into());
      }
    }

    // Construct bundlers from options and plugins
    let bundlers = options_and_plugins
      .into_iter()
      .map(|(options, plugins)| {
        Bundler::with_plugins(options, plugins)
          .map(|bundler| Arc::new(Mutex::new(bundler)))
      })
      .collect::<BuildResult<Vec<_>>>()?;

    let watcher = Arc::new(WatcherImpl::new(bundlers, notify_option)?);

    Ok(Self(watcher))
  }

  pub async fn start(&self) {
    wait_for_change(Arc::clone(&self.0));
    self.0.start().await;
  }

  pub async fn close(&self) -> Result<()> {
    self.0.close().await
  }

  pub fn emitter(&self) -> Arc<crate::watch::emitter::WatcherEmitter> {
    Arc::clone(&self.0.emitter)
  }
}
