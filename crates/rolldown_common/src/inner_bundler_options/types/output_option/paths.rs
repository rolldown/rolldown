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
        // Hitting this branch means a caller attempted to use the sync API on an async
        // function-based configuration (e.g. `options.paths` instead of `resolved_paths`).
        // Make this misuse visible in debug builds while preserving release behavior.
        debug_assert!(
          false,
          "PathsOutputOption::call was used on the `Fn` variant. \
           Use `resolve_all()` (or `resolved_paths`) to pre-resolve async paths before sync access."
        );
        None
      }
    }
  }

  /// Pre-resolve all paths for the given IDs asynchronously, returning a `FxHashMap` variant.
  pub async fn resolve_all<'a>(&self, ids: impl Iterator<Item = &'a str>) -> PathsOutputOption {
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
