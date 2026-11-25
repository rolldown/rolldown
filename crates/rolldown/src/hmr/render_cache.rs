use std::{
  collections::VecDeque,
  hash::{Hash, Hasher},
  sync::Arc,
};

use arcstr::ArcStr;
use rolldown_common::{IndexModules, Module, ModuleIdx, NormalModule};
use rolldown_sourcemap::{Source, SourceMap};
use rustc_hash::{FxHashMap, FxHasher};

use crate::types::scan_stage_cache::ScanStageCache;

/// The cache drops the least recently used factories when its estimated size goes above this.
const MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;
/// About the size of one sourcemap token in memory.
const TOKEN_BYTES: usize = 32;
/// The cache keeps at most this many finished payloads, and at most `MAX_PAYLOAD_BYTES` of them.
const MAX_PAYLOADS: usize = 8;
const MAX_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;

/// One module's factory, rendered for a payload: the code between its `//#region` lines.
#[derive(Debug)]
pub struct RenderedFactory {
  pub code: String,
  pub map: Option<SourceMap>,
}

impl RenderedFactory {
  fn estimated_bytes(&self) -> usize {
    let map_bytes = self.map.as_ref().map_or(0, |map| {
      map.get_tokens().len() * TOKEN_BYTES
        + map.get_names().map(str::len).sum::<usize>()
        + map.get_sources().flatten().map(str::len).sum::<usize>()
        + map.get_source_contents().flatten().map(str::len).sum::<usize>()
    });
    self.code.len() + map_bytes
  }
}

/// A source that borrows a cached factory, so a payload does not copy it before the join.
pub struct CachedFactorySource(pub Arc<RenderedFactory>);

impl Source for CachedFactorySource {
  fn sourcemap(&self) -> Option<&SourceMap> {
    self.0.map.as_ref()
  }

  fn content(&self) -> &str {
    &self.0.code
  }
}

/// Everything a pulled payload is made of. Two requests with the same key get the same code and
/// sourcemap, as long as the factories they name stay in the cache. Every payload prepares its
/// sourcemap for the same file name, so the name is not part of the key.
#[derive(Debug, PartialEq, Eq)]
pub struct PayloadKey {
  /// The `G` of the payload's `beginPayload(G)` prelude.
  pub generation: u32,
  pub catch_up: Option<String>,
  pub rows: Option<String>,
  /// The stable id, stamp and `render_inputs` hash of each factory, in payload order.
  pub factories: Vec<(ArcStr, u32, u64)>,
}

/// A payload as the engine returns it: the code, and the sourcemap as JSON.
#[derive(Debug)]
pub struct FinishedPayload {
  pub code: String,
  pub sourcemap: Option<String>,
}

impl FinishedPayload {
  fn bytes(&self) -> usize {
    self.code.len() + self.sourcemap.as_ref().map_or(0, String::len)
  }
}

/// How many lookups the cache answered since the `Bundler` was created.
#[cfg(feature = "testing")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderCacheHits {
  pub factories: u64,
  pub payloads: u64,
}

struct Entry {
  stamp: u32,
  inputs: u64,
  factory: Arc<RenderedFactory>,
  bytes: usize,
  last_use: u64,
}

/// Rendered HMR factories, so many clients that ask for the same module get one render.
///
/// One entry per module stable id. An entry is used only when its stamp and its `render_inputs`
/// hash match the module as it is now. The stamp covers the module's own code. The hash covers
/// what the factory reads about the modules it imports. A scan that reads the module again
/// removes its entry, because the new read can change the module without a new stamp.
///
/// The bundler options are fixed for the life of a `Bundler`, so they are not part of the key.
/// A full scan, a full reload of the clients and a new session clear the cache. See
/// internal-docs/hmr/design.md, "Render cache".
///
/// It also keeps the last finished payloads. Tabs in the same state ask for the same payload,
/// and the join of the factories and the sourcemap JSON then run once. A module read again can
/// change its sourcemap under the same stamp, so `remove` and `forget` drop the payloads too. An
/// insert or an eviction keeps them: the payload key names each factory's stamp and inputs.
#[derive(Default)]
pub struct HmrRenderCache {
  entries: FxHashMap<ArcStr, Entry>,
  bytes: usize,
  clock: u64,
  payloads: VecDeque<(PayloadKey, Arc<FinishedPayload>)>,
  payload_bytes: usize,
  #[cfg(feature = "testing")]
  hits: RenderCacheHits,
}

impl HmrRenderCache {
  pub fn get(&mut self, stable_id: &str, stamp: u32, inputs: u64) -> Option<Arc<RenderedFactory>> {
    let entry = self.entries.get_mut(stable_id)?;
    if entry.stamp != stamp || entry.inputs != inputs {
      return None;
    }
    self.clock += 1;
    entry.last_use = self.clock;
    #[cfg(feature = "testing")]
    {
      self.hits.factories += 1;
    }
    Some(Arc::clone(&entry.factory))
  }

  pub fn insert(
    &mut self,
    stable_id: ArcStr,
    stamp: u32,
    inputs: u64,
    factory: Arc<RenderedFactory>,
  ) {
    let bytes = factory.estimated_bytes();
    self.remove_entry(&stable_id);
    if bytes > MAX_CACHE_BYTES / 4 {
      return;
    }
    self.clock += 1;
    self.bytes += bytes;
    self.entries.insert(stable_id, Entry { stamp, inputs, factory, bytes, last_use: self.clock });
    if self.bytes > MAX_CACHE_BYTES {
      self.evict_to(MAX_CACHE_BYTES / 4 * 3);
    }
  }

  pub fn remove(&mut self, stable_id: &str) {
    // Also when the factory was not cached: a payload can carry a factory too large to keep.
    self.clear_payloads();
    self.remove_entry(stable_id);
  }

  pub fn clear(&mut self) {
    self.entries.clear();
    self.bytes = 0;
    self.clear_payloads();
  }

  /// Call before a partial scan that reads `modules` again. It removes their factories: a
  /// module read again can change without a new stamp. Example: an edit that moves code down
  /// one line renders the same code, so the HMR stage stamps nothing, but the sourcemap changes.
  pub fn forget(&mut self, cache: &ScanStageCache, modules: impl IntoIterator<Item = ModuleIdx>) {
    if !cache.has_snapshot() {
      self.clear();
      return;
    }
    let module_table = &cache.get_snapshot().module_table;
    for module_idx in modules {
      if let Some(module) = module_table.modules.get(module_idx) {
        self.remove(module.stable_id().as_str());
      }
    }
  }

  /// Like `forget`, for a scan that also reads again the files an earlier failed scan left in
  /// `pending_rescans`.
  pub fn forget_rescanned(
    &mut self,
    cache: &ScanStageCache,
    modules: impl IntoIterator<Item = ModuleIdx>,
  ) {
    let pending = cache.pending_rescans.iter().filter_map(|resolved_id| {
      cache.module_id_to_idx.get(&resolved_id.id).map(|state| state.idx())
    });
    self.forget(cache, modules.into_iter().chain(pending));
  }

  pub fn get_payload(&mut self, key: &PayloadKey) -> Option<Arc<FinishedPayload>> {
    let index = self.payloads.iter().position(|(cached_key, _)| cached_key == key)?;
    let entry = self.payloads.remove(index)?;
    let payload = Arc::clone(&entry.1);
    self.payloads.push_back(entry);
    #[cfg(feature = "testing")]
    {
      self.hits.payloads += 1;
    }
    Some(payload)
  }

  pub fn insert_payload(&mut self, key: PayloadKey, code: &str, sourcemap: Option<&str>) {
    let bytes = code.len() + sourcemap.map_or(0, str::len);
    if bytes > MAX_PAYLOAD_BYTES / 2 {
      return;
    }
    let payload =
      FinishedPayload { code: code.to_owned(), sourcemap: sourcemap.map(ToOwned::to_owned) };
    self.payload_bytes += bytes;
    self.payloads.push_back((key, Arc::new(payload)));
    while self.payloads.len() > MAX_PAYLOADS || self.payload_bytes > MAX_PAYLOAD_BYTES {
      let Some((_, oldest)) = self.payloads.pop_front() else { break };
      self.payload_bytes -= oldest.bytes();
    }
  }

  #[cfg(feature = "testing")]
  pub fn hits(&self) -> RenderCacheHits {
    self.hits
  }

  fn remove_entry(&mut self, stable_id: &str) {
    if let Some(entry) = self.entries.remove(stable_id) {
      self.bytes -= entry.bytes;
    }
  }

  fn clear_payloads(&mut self) {
    self.payloads.clear();
    self.payload_bytes = 0;
  }

  /// Drops the least recently used entries until the size is at most `target`.
  fn evict_to(&mut self, target: usize) {
    let mut by_last_use =
      self.entries.iter().map(|(id, entry)| (entry.last_use, id.clone())).collect::<Vec<_>>();
    by_last_use.sort_unstable_by_key(|(last_use, _)| *last_use);
    for (_, id) in by_last_use {
      if self.bytes <= target {
        break;
      }
      self.remove_entry(&id);
    }
  }

  #[cfg(test)]
  fn len(&self) -> usize {
    self.entries.len()
  }
}

/// A hash of what the factory of `module` reads about the modules it imports: their ids and
/// names, and the facts that choose the interop code. The module's stamp does not cover these
/// when an import resolves to another module or an importee changes its kind.
pub fn render_inputs(modules: &IndexModules, module: &NormalModule) -> u64 {
  let mut hasher = FxHasher::default();
  for record in &module.import_records {
    let Some(importee_idx) = record.resolved_module else {
      0u8.hash(&mut hasher);
      continue;
    };
    match &modules[importee_idx] {
      Module::Normal(importee) => {
        1u8.hash(&mut hasher);
        importee.id.as_str().hash(&mut hasher);
        importee.stable_id.as_str().hash(&mut hasher);
        importee.repr_name.as_str().hash(&mut hasher);
        std::mem::discriminant(&importee.exports_kind).hash(&mut hasher);
        importee.meta.has_lazy_export().hash(&mut hasher);
      }
      Module::External(importee) => {
        2u8.hash(&mut hasher);
        importee.id.as_str().hash(&mut hasher);
        importee.stable_id.as_str().hash(&mut hasher);
        importee.identifier_name.as_str().hash(&mut hasher);
      }
    }
  }
  hasher.finish()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn factory(code_len: usize) -> Arc<RenderedFactory> {
    Arc::new(RenderedFactory { code: "x".repeat(code_len), map: None })
  }

  #[test]
  fn an_entry_needs_the_same_stamp_and_inputs() {
    let mut cache = HmrRenderCache::default();
    cache.insert("a.js".into(), 2, 7, factory(10));
    assert!(cache.get("a.js", 2, 7).is_some());
    assert!(cache.get("a.js", 3, 7).is_none());
    assert!(cache.get("a.js", 2, 8).is_none());
    cache.remove("a.js");
    assert!(cache.get("a.js", 2, 7).is_none());
    assert_eq!(cache.bytes, 0);
  }

  #[test]
  fn the_least_recently_used_entries_go_first() {
    let mut cache = HmrRenderCache::default();
    let size = MAX_CACHE_BYTES / 8;
    for i in 0..8 {
      cache.insert(format!("{i}.js").into(), 0, 0, factory(size));
    }
    // `0.js` is used again, so `1.js` is now the oldest.
    assert!(cache.get("0.js", 0, 0).is_some());
    cache.insert("8.js".into(), 0, 0, factory(size));
    assert!(cache.bytes <= MAX_CACHE_BYTES);
    assert!(cache.get("0.js", 0, 0).is_some());
    assert!(cache.get("1.js", 0, 0).is_none());
    assert!(cache.get("8.js", 0, 0).is_some());
    assert_eq!(cache.len(), 6);
  }

  fn payload_key(stamp: u32) -> PayloadKey {
    PayloadKey {
      generation: 2,
      catch_up: None,
      rows: Some("rows".to_owned()),
      factories: vec![("a.js".into(), stamp, 7)],
    }
  }

  #[test]
  fn a_payload_needs_the_same_key_and_goes_on_a_removal() {
    let mut cache = HmrRenderCache::default();
    cache.insert("a.js".into(), 2, 7, factory(10));
    cache.insert_payload(payload_key(2), "code", None);
    assert_eq!(cache.get_payload(&payload_key(2)).unwrap().code, "code");
    assert!(cache.get_payload(&payload_key(3)).is_none());

    // A scan that reads any module again removes its factory.
    cache.remove("b.js");
    assert!(cache.get_payload(&payload_key(2)).is_none());
    assert_eq!(cache.payload_bytes, 0);

    cache.insert_payload(payload_key(2), "code", None);
    cache.clear();
    assert!(cache.get_payload(&payload_key(2)).is_none());
  }

  #[test]
  fn an_insert_or_an_eviction_keeps_the_payloads() {
    let mut cache = HmrRenderCache::default();
    cache.insert("a.js".into(), 2, 7, factory(10));
    cache.insert_payload(payload_key(2), "code", None);

    // Another tab's pull renders a newer `a.js`, and a large factory pushes `a.js` out.
    cache.insert("a.js".into(), 3, 7, factory(10));
    cache.insert("b.js".into(), 0, 0, factory(MAX_CACHE_BYTES / 8));
    for i in 0..8 {
      cache.insert(format!("{i}.js").into(), 0, 0, factory(MAX_CACHE_BYTES / 8));
    }
    assert!(cache.get("b.js", 0, 0).is_none());
    assert_eq!(cache.get_payload(&payload_key(2)).unwrap().code, "code");
  }

  #[test]
  fn a_payload_of_another_generation_is_not_shared() {
    let mut cache = HmrRenderCache::default();
    cache.insert_payload(payload_key(2), "code", None);
    // The same rows and factories, rendered after a rebuild that changed none of them.
    let next = PayloadKey { generation: 3, ..payload_key(2) };
    assert!(cache.get_payload(&next).is_none());
  }

  #[test]
  fn the_least_recently_used_payloads_go_first() {
    let mut cache = HmrRenderCache::default();
    for stamp in 0..u32::try_from(MAX_PAYLOADS).unwrap() {
      cache.insert_payload(payload_key(stamp), "code", None);
    }
    // `0` is used again, so `1` is now the oldest.
    assert!(cache.get_payload(&payload_key(0)).is_some());
    cache.insert_payload(payload_key(100), "code", None);
    assert!(cache.get_payload(&payload_key(0)).is_some());
    assert!(cache.get_payload(&payload_key(1)).is_none());
    assert!(cache.get_payload(&payload_key(100)).is_some());
    assert_eq!(cache.payloads.len(), MAX_PAYLOADS);
  }

  #[test]
  fn a_too_large_factory_is_not_kept() {
    let mut cache = HmrRenderCache::default();
    cache.insert("a.js".into(), 0, 0, factory(MAX_CACHE_BYTES / 2));
    assert!(cache.get("a.js", 0, 0).is_none());
    assert_eq!(cache.bytes, 0);
  }
}
