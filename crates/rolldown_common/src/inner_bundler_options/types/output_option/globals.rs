use std::{collections::HashMap, fmt::Debug, future::Future, pin::Pin, sync::Arc};

use rustc_hash::FxHashMap;

pub type GlobalsFunction = dyn Fn(&str) -> Pin<Box<(dyn Future<Output = anyhow::Result<String>> + Send + 'static)>>
  + Send
  + Sync;

#[derive(Clone)]
pub enum GlobalsOutputOption {
  FxHashMap(FxHashMap<String, String>),
  Fn(Arc<GlobalsFunction>),
}

impl Debug for GlobalsOutputOption {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::FxHashMap(value) => write!(f, "GlobalsOutputOption::FxHashMap({value:?})"),
      Self::Fn(_) => write!(f, "GlobalsOutputOption::Fn(...)"),
    }
  }
}

impl GlobalsOutputOption {
  pub async fn call(&self, name: &str) -> Option<String> {
    match self {
      Self::FxHashMap(value) => value.get(name).cloned(),
      Self::Fn(value) => value(name).await.ok(),
    }
  }
}

impl From<HashMap<String, String>> for GlobalsOutputOption {
  fn from(value: HashMap<String, String>) -> Self {
    Self::FxHashMap(value.into_iter().collect())
  }
}
