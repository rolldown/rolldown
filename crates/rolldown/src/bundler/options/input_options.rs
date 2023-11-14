use std::fmt::Debug;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use derivative::Derivative;
use futures::Future;
use rolldown_error::BuildError;

pub type ExternalFn = dyn Fn(
    String,
    Option<String>,
    bool,
  ) -> Pin<Box<(dyn Future<Output = Result<bool, BuildError>> + Send + 'static)>>
  + Send
  + Sync;

pub enum External {
  ArrayString(Vec<String>),
  Fn(Box<ExternalFn>),
}

impl Debug for External {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ArrayString(value) => write!(f, "External::ArrayString({value:?})"),
      Self::Fn(_) => write!(f, "External::Fn(...)"),
    }
  }
}

impl Default for External {
  fn default() -> Self {
    Self::ArrayString(vec![])
  }
}

impl External {
  pub async fn call(
    &self,
    source: String,
    importer: Option<String>,
    is_resolved: bool,
  ) -> Result<bool, BuildError> {
    match self {
      Self::ArrayString(value) => {
        let result = value.iter().any(|item| item == &source);
        Ok(result)
      }
      Self::Fn(value) => value(source, importer, is_resolved).await,
    }
  }
}

#[derive(Debug)]
pub struct InputItem {
  pub name: Option<String>,
  pub import: String,
}

impl From<String> for InputItem {
  fn from(value: String) -> Self {
    Self { name: None, import: value }
  }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct InputOptions {
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub external: External,
}

impl Default for InputOptions {
  fn default() -> Self {
    Self { input: vec![], cwd: std::env::current_dir().unwrap(), external: External::default() }
  }
}

pub type SharedInputOptions = Arc<InputOptions>;
