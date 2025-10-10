use derive_more::Debug;
use std::{future::Future, pin::Pin, sync::Arc};

use rustc_hash::FxHashMap;

pub type PathsFunction = dyn Fn(&str) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
pub enum PathsOutputOption {
  #[debug("PathsOutputOption::FxHashMap({_0:?})")]
  FxHashMap(FxHashMap<String, String>),
  #[debug("PathsOutputOption::Fn(...)")]
  Fn(Arc<PathsFunction>),
}

impl PathsOutputOption {
  pub async fn call(&self, id: &str) -> Option<String> {
    match self {
      Self::FxHashMap(value) => value.get(id).cloned(),
      Self::Fn(value) => value(id).await.ok(),
    }
  }
}

impl From<FxHashMap<String, String>> for PathsOutputOption {
  fn from(value: FxHashMap<String, String>) -> Self {
    Self::FxHashMap(value)
  }
}
