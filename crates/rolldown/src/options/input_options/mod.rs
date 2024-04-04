use std::fmt::Debug;
use std::path::PathBuf;
use std::pin::Pin;

use derivative::Derivative;
use futures::Future;
use rolldown_error::BuildError;

use self::resolve_options::ResolveOptions;

use super::types::input_item::InputItem;
use super::types::platform::Platform;

pub mod resolve_options;

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
      Self::ArrayString(value) => Ok(value.iter().any(|item| item == &source)),
      Self::Fn(value) => value(source, importer, is_resolved).await,
    }
  }
}

#[derive(Derivative, Default)]
#[derivative(Debug)]
pub struct InputOptions {
  pub input: Vec<InputItem>,
  pub cwd: Option<PathBuf>,
  pub external: Option<External>,
  pub treeshake: Option<bool>,
  pub resolve: Option<ResolveOptions>,
  pub platform: Option<Platform>,
}
