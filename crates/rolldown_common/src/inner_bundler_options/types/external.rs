use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

pub type ExternalFn = dyn Fn(
    String,
    Option<String>,
    bool,
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<bool>> + Send + 'static)>>
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

impl External {
  pub async fn call(
    &self,
    source: String,
    importer: Option<String>,
    is_resolved: bool,
  ) -> anyhow::Result<bool> {
    match self {
      Self::ArrayString(value) => Ok(value.iter().any(|item| item == &source)),
      Self::Fn(value) => value(source, importer, is_resolved).await,
    }
  }
}
