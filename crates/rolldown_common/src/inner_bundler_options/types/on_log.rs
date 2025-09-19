use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

use super::log_level::LogLevel;

pub type OnLogFn = dyn Fn(LogLevel, Log) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
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

#[derive(Debug, Default)]
pub struct Log {
  pub message: String,
  pub id: Option<String>,
  pub code: Option<String>,
  pub exporter: Option<String>,
  pub plugin: Option<String>,
}

#[derive(Debug, Default)]
pub struct LogWithoutPlugin {
  pub message: String,
  pub id: Option<String>,
  pub code: Option<String>,
  pub exporter: Option<String>,
}

impl LogWithoutPlugin {
  pub fn into_log(self, plugin_name: Option<String>) -> Log {
    Log {
      message: self.message,
      id: self.id,
      code: self.code,
      exporter: self.exporter,
      plugin: plugin_name,
    }
  }
}
