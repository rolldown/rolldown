use std::sync::Arc;

use arcstr::ArcStr;

use rolldown_common::OutputChunk;
use rolldown_plugin::typedmap::TypedMapKey;
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use rustc_hash::FxHashMap;

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
  pub imported_css: FxDashSet<ArcStr>,
  pub imported_assets: FxDashSet<ArcStr>,
}

#[derive(Debug, Default)]
pub struct ViteMetadata {
  pub inner: FxDashMap<ArcStr, Arc<ChunkMetadata>>,
}

impl ViteMetadata {
  pub fn get(&self, key: &ArcStr) -> Option<Arc<ChunkMetadata>> {
    self.inner.get(key).map(|v| v.clone())
  }

  pub fn get_or_insert_default(&self, key: ArcStr) -> Arc<ChunkMetadata> {
    self.inner.entry(key).or_insert_with(|| Arc::new(ChunkMetadata::default())).clone()
  }
}

#[derive(Debug, Default)]
pub struct CSSEntriesCache {
  pub inner: FxDashMap<ArcStr, ArcStr>,
}

#[derive(Debug, Default)]
pub struct CSSModuleCache {
  pub inner: FxDashMap<String, FxHashMap<String, String>>,
}

#[derive(Debug, Default)]
pub struct HTMLProxyResult {
  pub inner: FxDashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct HTMLProxyMapItem {
  pub code: ArcStr,
  pub map: Option<rolldown_sourcemap::SourceMap>,
}

#[derive(Debug, Default)]
pub struct HTMLProxyMap {
  pub inner: FxDashMap<String, FxDashMap<usize, HTMLProxyMapItem>>,
}

#[derive(Debug, Default)]
pub struct CSSStyles {
  pub inner: FxDashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct PureCSSChunks {
  pub inner: FxDashSet<ArcStr>,
}

#[derive(Debug, Default)]
pub struct CSSChunkCache {
  pub inner: FxDashMap<ArcStr, String>,
}

#[derive(Debug, Default)]
pub struct CSSBundleName(pub String);

#[derive(Debug, Default)]
pub struct RemovedPureCSSFilesCache {
  pub inner: FxDashMap<ArcStr, Arc<OutputChunk>>,
}
