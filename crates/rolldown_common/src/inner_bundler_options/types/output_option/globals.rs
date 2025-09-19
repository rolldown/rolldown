use derive_more::Debug;
use std::{future::Future, pin::Pin, sync::Arc};

use rustc_hash::FxHashMap;

pub type GlobalsFunction = dyn Fn(&str) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
pub enum GlobalsOutputOption {
  #[debug("GlobalsOutputOption::FxHashMap({_0:?})")]
  FxHashMap(FxHashMap<String, String>),
  #[debug("GlobalsOutputOption::Fn(...)")]
  Fn(Arc<GlobalsFunction>),
}

impl GlobalsOutputOption {
  pub async fn call(&self, name: &str) -> Option<String> {
    match self {
      Self::FxHashMap(value) => value.get(name).cloned(),
      Self::Fn(value) => value(name).await.ok(),
    }
  }
}

impl From<FxHashMap<String, String>> for GlobalsOutputOption {
  fn from(value: FxHashMap<String, String>) -> Self {
    Self::FxHashMap(value)
  }
}
