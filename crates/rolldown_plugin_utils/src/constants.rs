use std::sync::Arc;

use arcstr::ArcStr;

use rolldown_plugin::typedmap::TypedMapKey;
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};

// Use `10kB` as a threshold for 'auto'
// https://v8.dev/blog/cost-of-javascript-2019#json
pub const THRESHOLD_SIZE: usize = 10 * 1000;

#[derive(Hash, PartialEq, Eq)]
pub struct ViteImportGlob;
pub struct ViteImportGlobValue(pub bool);

impl ViteImportGlobValue {
  pub fn is_sub_imports_pattern(&self) -> bool {
    self.0
  }
}

impl TypedMapKey for ViteImportGlob {
  type Value = ViteImportGlobValue;
}

#[derive(Debug, Default)]
pub struct ChunkMetadata {
  pub imported_assets: FxDashSet<ArcStr>,
}

#[derive(Debug, Default)]
pub struct ViteMetadata {
  pub inner: FxDashMap<ArcStr, Arc<ChunkMetadata>>,
}

impl ViteMetadata {
  pub fn get(&self, key: ArcStr) -> Arc<ChunkMetadata> {
    self.inner.entry(key).or_default().clone()
  }
}
