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
  /// Sync call — only works for the `FxHashMap` variant.
  /// The `Fn` variant should be pre-resolved via `resolve_all()` before sync access.
  pub fn call(&self, id: &str) -> Option<String> {
    match self {
      Self::FxHashMap(value) => value.get(id).cloned(),
      Self::Fn(_) => {
        // The Fn variant should be pre-resolved via `resolve_all()` before sync access.
        // If reached, the path was not pre-resolved — return None as a safe fallback.
        None
      }
    }
  }

  /// Pre-resolve all paths for the given IDs asynchronously, returning a `FxHashMap` variant.
  pub async fn resolve_all<'a>(
    &self,
    ids: impl Iterator<Item = &'a str>,
  ) -> PathsOutputOption {
    match self {
      Self::FxHashMap(map) => Self::FxHashMap(map.clone()),
      Self::Fn(f) => {
        let mut resolved = FxHashMap::default();
        for id in ids {
          if let Ok(path) = f(id).await {
            resolved.insert(id.to_string(), path);
          }
        }
        Self::FxHashMap(resolved)
      }
    }
  }
}

impl From<FxHashMap<String, String>> for PathsOutputOption {
  fn from(value: FxHashMap<String, String>) -> Self {
    Self::FxHashMap(value)
  }
}
