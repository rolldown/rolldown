use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use dashmap::Entry;
use napi_derive::napi;
use oxc_resolver::{
  ResolveError, ResolveOptions, Resolver, TsConfig, TsconfigDiscovery, TsconfigOptions,
  TsconfigReferences,
};
use rolldown_utils::dashmap::FxDashMap;

#[napi]
pub struct TsconfigCache {
  resolver: Arc<Resolver>,
  cache: FxDashMap<PathBuf, Arc<TsConfig>>,
}

#[napi]
impl TsconfigCache {
  /// Create a new transform cache with auto or manual tsconfig discovery enabled.
  #[napi(constructor)]
  pub fn new(yarn_pnp: bool, path_to_tsconfig: Option<String>) -> Self {
    Self {
      resolver: Arc::new(Resolver::new(ResolveOptions {
        tsconfig: Some(path_to_tsconfig.map_or(TsconfigDiscovery::Auto, |config_file| {
          TsconfigDiscovery::Manual(TsconfigOptions {
            config_file: PathBuf::from(config_file),
            references: TsconfigReferences::Auto,
          })
        })),
        yarn_pnp,
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
    self.resolver.clear_cache();
    self.cache.clear();
  }

  /// Get the number of cached entries.
  #[napi]
  pub fn size(&self) -> u32 {
    u32::try_from(self.cache.len()).unwrap_or(u32::MAX)
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
    let cache = TsconfigCache::new(false, None);
    assert_eq!(cache.size(), 0);
  }

  #[test]
  fn test_cache_clear() {
    let cache = TsconfigCache::new(false, None);
    cache.clear();
    assert_eq!(cache.size(), 0);
  }
}
