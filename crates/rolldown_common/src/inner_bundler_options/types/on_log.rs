use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;
use rolldown_error::{BuildDiagnostic, RenderedDiagnostic};

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

#[derive(Debug, Default, Clone)]
pub struct LogLocation {
  /// 1-based
  pub line: u32,
  /// 0-based position in the line in UTF-16 code units
  pub column: u32,
  pub file: Option<String>,
}

#[derive(Debug, Default)]
pub struct Log {
  pub message: String,
  pub id: Option<String>,
  pub code: Option<String>,
  pub exporter: Option<String>,
  pub plugin: Option<String>,
  pub loc: Option<LogLocation>,
  pub pos: Option<u32>,
  pub ids: Option<Vec<String>>,
}

impl Log {
  pub fn from_rendered(warning: &BuildDiagnostic, rendered: RenderedDiagnostic) -> Self {
    #[expect(
      clippy::cast_possible_truncation,
      reason = "line/column/position values are unlikely to exceed u32::MAX in practical use"
    )]
    let (loc, pos) = match rendered.primary_location {
      Some(location) => (
        Some(LogLocation {
          line: location.line as u32,
          column: location.column as u32,
          // Use warning.id() since the diagnostic may only store the filename.
          file: warning.id(),
        }),
        Some(location.utf16_position as u32),
      ),
      None => (None, None),
    };

    Self {
      id: warning.id(),
      exporter: warning.exporter(),
      code: Some(warning.kind().to_string()),
      message: rendered.message,
      plugin: warning.plugin(),
      loc,
      pos,
      ids: warning.ids(),
    }
  }
}

#[derive(Debug, Default)]
pub struct LogWithoutPlugin {
  pub message: String,
  pub id: Option<String>,
  pub code: Option<String>,
  pub exporter: Option<String>,
  pub loc: Option<LogLocation>,
  pub pos: Option<u32>,
  pub ids: Option<Vec<String>>,
}

impl LogWithoutPlugin {
  pub fn into_log(self, plugin_name: Option<String>) -> Log {
    Log {
      message: self.message,
      id: self.id,
      code: self.code,
      exporter: self.exporter,
      plugin: plugin_name,
      loc: self.loc,
      pos: self.pos,
      ids: self.ids,
    }
  }
}
