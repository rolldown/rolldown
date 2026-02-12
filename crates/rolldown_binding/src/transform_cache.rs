use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use dashmap::Entry;
use napi_derive::napi;
use oxc_resolver::{ResolveError, ResolveOptions, Resolver, TsConfig, TsconfigDiscovery};
use rolldown_utils::dashmap::FxDashMap;

/// Cache for tsconfig resolution to avoid redundant file system operations.
///
/// The cache stores resolved tsconfig configurations keyed by their file paths.
/// When transforming multiple files in the same project, tsconfig lookups are
/// deduplicated, improving performance.
#[napi]
pub struct TsconfigCache {
  resolver: Arc<Resolver>,
  cache: FxDashMap<PathBuf, Arc<TsConfig>>,
}

#[napi]
impl TsconfigCache {
  /// Create a new transform cache with auto tsconfig discovery enabled.
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {
      resolver: Arc::new(Resolver::new(ResolveOptions {
        tsconfig: Some(TsconfigDiscovery::Auto),
        ..Default::default()
      })),
      cache: FxDashMap::default(),
    }
  }

  /// Clear the cache.
  ///
  /// Call this when tsconfig files have changed to ensure fresh resolution.
  #[napi]
  pub fn clear(&self) {
    self.cache.clear();
  }

  /// Get the number of cached entries.
  #[napi]
  pub fn size(&self) -> u32 {
    u32::try_from(self.cache.len()).unwrap_or(u32::MAX)
  }
}

impl Default for TsconfigCache {
  fn default() -> Self {
    Self::new()
  }
}

impl TsconfigCache {
  /// Get the resolver instance.
  pub fn resolver(&self) -> &Resolver {
    &self.resolver
  }

  /// Find and cache tsconfig for a given file path.
  ///
  /// Returns None if no tsconfig is found for the file.
  pub fn find_tsconfig(&self, file_path: &Path) -> Result<Option<Arc<TsConfig>>, ResolveError> {
    let tsconfig_result = self.resolver.find_tsconfig(file_path);
    match tsconfig_result {
      Ok(Some(arc_tsconfig)) => {
        let cache_key = arc_tsconfig.path.clone();

        match self.cache.entry(cache_key) {
          Entry::Occupied(entry) => Ok(Some(Arc::clone(entry.get()))),
          Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(Arc::clone(&arc_tsconfig));
            Ok(Some(arc_tsconfig))
          }
        }
      }
      Ok(None) | Err(_) => tsconfig_result,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_cache_creation() {
    let cache = TsconfigCache::new();
    assert_eq!(cache.size(), 0);
  }

  #[test]
  fn test_cache_clear() {
    let cache = TsconfigCache::new();
    cache.clear();
    assert_eq!(cache.size(), 0);
  }
}
