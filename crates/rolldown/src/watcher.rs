use std::sync::Arc;

use rolldown_error::{BuildDiagnostic, BuildResult};
use tokio::sync::Mutex;

use crate::{
  BundlerBuilder,
  types::bundler_config::BundlerConfig,
  watch::watcher::{WatcherImpl, wait_for_change},
};

pub struct Watcher(Arc<WatcherImpl>);

impl Watcher {
  pub fn new(config: BundlerConfig) -> BuildResult<Self> {
    Self::with_configs(vec![config])
  }

  pub fn with_configs(configs: Vec<BundlerConfig>) -> BuildResult<Self> {
    let mut bundlers = Vec::with_capacity(configs.len());

    for config in configs {
      // Validation: dev_mode not allowed with watch
      if config.options.experimental.as_ref().and_then(|e| e.dev_mode.as_ref()).is_some() {
        return Err(
          BuildDiagnostic::bundler_initialize_error(
            "The \"experimental.devMode\" option is only supported with the \"dev\" API. \
             It cannot be used with \"watch\". Please use the \"dev\" API for dev mode functionality."
              .to_string(),
            None,
          )
          .into(),
        );
      }

      // Build the bundler from config
      let bundler = BundlerBuilder::default()
        .with_options(config.options)
        .with_plugins(config.plugins)
        .build()?;

      bundlers.push(Arc::new(Mutex::new(bundler)));
    }

    let watcher = Arc::new(WatcherImpl::new(bundlers)?);
    Ok(Self(watcher))
  }

  pub async fn start(&self) {
    wait_for_change(Arc::clone(&self.0));
    self.0.start().await;
  }

  pub async fn close(&self) -> anyhow::Result<()> {
    self.0.close().await
  }

  pub fn emitter(&self) -> Arc<crate::watch::emitter::WatcherEmitter> {
    Arc::clone(&self.0.emitter)
  }
}
