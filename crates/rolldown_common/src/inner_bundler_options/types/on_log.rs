use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

use super::log_level::LogLevel;

pub type OnLogFn = dyn Fn(LogLevel, Log) -> Pin<Box<(dyn Future<Output = anyhow::Result<()>> + Send + 'static)>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
#[debug("OnLogFn::Fn(...)")]
pub struct OnLog(Arc<OnLogFn>);

impl OnLog {
  pub fn new(f: Arc<OnLogFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, log_level: LogLevel, log: Log) -> anyhow::Result<()> {
    self.0(log_level, log).await
  }
}

pub struct Log {
  pub code: String,
  pub message: String,
  pub id: Option<String>,
  pub exporter: Option<String>,
}
