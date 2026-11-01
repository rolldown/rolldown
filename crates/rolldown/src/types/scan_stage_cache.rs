use std::sync::Arc;

use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  BarrelState, EcmaModuleAstUsage, GetLocalDbMut, ImporterRecord, Module, ModuleId, ModuleIdx,
  OutputAsset, OutputChunk, ResolvedId, RollupRenderedChunk, StableModuleId,
};
use rolldown_error::BuildResult;
use rolldown_plugin::PluginDriver;
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::{
  SharedOptions,
  module_loader::{deferred_scan_data::defer_sync_scan_data, module_loader::VisitState},
  stages::scan_stage::{NormalizedScanStageOutput, ScanStageOutput},
};

#[derive(Clone, Debug)]
pub struct CachedChunkNaming {
  pub scan_generation: u64,
  pub input_base: ArcStr,
  pub explicit_name: Option<ArcStr>,
  pub is_user_defined_entry: bool,
  pub used_name_keys: Option<Vec<ArcStr>>,
  pub representative_chunk_name: ArcStr,
  pub chunk_name: ArcStr,
  pub chunk_filename: ArcStr,
  pub preliminary_filename: rolldown_common::PreliminaryFilename,
  pub absolute_preliminary_filename: String,
  pub preliminary_sourcemap_filename: Option<rolldown_common::PreliminaryFilename>,
}

#[derive(Clone, Default)]
pub struct CachedModuleSources(
  pub Option<Arc<[Box<dyn rolldown_sourcemap::Source + Send + Sync>]>>,
);

impl std::fmt::Debug for CachedModuleSources {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("CachedModuleSources").field(&self.0.is_some()).finish()
  }
}

#[derive(Clone, Debug)]
pub struct CachedFinalizedModule {
  pub digest: [u128; 2],
  pub sources: CachedModuleSources,
}

#[derive(Clone, Debug)]
pub struct CachedChunkSourcemap {
  pub filename: ArcStr,
  pub pre_minify_hash: [u8; 32],
  pub pre_minify_placeholders: Vec<String>,
  pub post_minify_content: Option<ArcStr>,
  pub json: ArcStr,
  pub names: Vec<String>,
  pub original_file_names: Vec<String>,
  pub pre_redacted: bool,
}

pub type SourcemapContentFilter =
  Arc<dyn Fn(&str, &str) -> anyhow::Result<bool> + Send + Sync>;

#[derive(Clone)]
pub struct SourcemapContentFilterHolder(pub SourcemapContentFilter);

impl std::fmt::Debug for SourcemapContentFilterHolder {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("SourcemapContentFilter")
  }
}

/// The code of a chunk, keyed by a digest of everything that code is joined from: the digests of
/// its modules' rendered code in order, and the code the format renders around them (imports,
/// exports, addons). A rebuild that renders the same key reuses the code instead of joining the
/// chunk's modules again.
#[derive(Clone, Debug)]
pub struct CachedMultiModuleChunk {
  /// Chunk code before hash placeholders resolve.
  pub content: Arc<str>,
  /// Whether joining the chunk produced a source map.
  pub had_map: bool,
  /// What finalizing derived from the code it last received for this chunk.
  pub finalized: Option<CachedChunkFinalize>,
}

/// What finalizing derived from a chunk's code, which a later finalize reuses when it receives
/// the same code.
#[derive(Clone, Debug)]
pub struct CachedChunkFinalize {
  /// The code finalizing received: the cached `content`, sharing its allocation, or what render
  /// hooks and minifying made of it.
  pub code: Arc<str>,
  /// The distinct hash placeholders in `code`, in order of first occurrence.
  pub placeholders: Arc<[String]>,
  /// The placeholder-normalized hash of `code`, with which of `placeholders` were chunks of the
  /// build that computed it.
  pub content_hash: Option<(Vec<bool>, String)>,
}

#[derive(Clone, Debug)]
pub struct CachedNativeChunk {
  pub decision_hash: [u128; 2],
  pub chunk: Arc<OutputChunk>,
  pub sourcemap_asset: Option<Arc<OutputAsset>>,
}

#[derive(Debug)]
pub struct CachedNativeRelinkState {
  pub link_output: crate::stages::link_stage::LinkStageOutput,
  pub generate_state: crate::stages::generate_stage::PendingNativeRelinkState,
  /// The side effects that each module's scan found, from which the link derived the linked
  /// side effects in `link_output`.
  pub scanned_side_effects:
    IndexVec<ModuleIdx, rolldown_common::side_effects::DeterminedSideEffects>,
}

/// The modules a fast native relink added to and retired from the output graph, by id.
#[derive(Debug, Default, Clone)]
pub struct NativeGraphChange {
  pub added: Vec<ArcStr>,
  pub retired: Vec<ArcStr>,
  /// Output files of retired records, which a host holding the previous output drops.
  pub retired_filenames: Vec<ArcStr>,
}

/// How the records of a link's output convert the exports of a CommonJS module that ES modules
/// import (`link_stage::commonjs_interop`, #660).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonjsInterop {
  /// Whether its importers' records pass its exports through `__toESM`, which one default or
  /// namespace import of it makes all of them do.
  pub converted: bool,
  /// The stable ids of the importers whose records read its exports through a binding.
  pub readers: std::collections::BTreeSet<String>,
}

pub type ModuleImportSignature = (
  Vec<(Option<ModuleIdx>, rolldown_common::ImportKind, u16)>,
  Vec<(rolldown_common::ImportRecordIdx, rolldown_common::Specifier)>,
);

/// Move-only scan data, without a filesystem, resolver, plugin driver or emitter.
/// Callers must validate the old revision's observations before transferring it.
#[derive(Debug)]
pub struct RevisionScanCache(pub(crate) ScanStageCache);

impl RevisionScanCache {
  pub fn contains_leaf(&self, id: &str) -> bool {
    self.0.contains_leaf(id)
  }

  pub fn can_partial_rescan(&self, id: &str) -> bool {
    self.0.can_partial_rescan(id)
  }

  pub fn module_import_signature(&self, id: &str) -> Option<ModuleImportSignature> {
    self.0.module_import_signature(id)
  }

  pub fn contains_module(&self, id: &str) -> bool {
    if self.0.module_id_to_idx.contains_key(id) {
      return true;
    }
    self.0.snapshot().is_some_and(|snapshot| {
      snapshot.module_table.modules.iter().any(|module| module.id().as_str() == id)
    })
  }

  pub fn modules_for_changed_paths(&self, changed_clean_paths: &[ArcStr]) -> Vec<ArcStr> {
    if changed_clean_paths.is_empty() {
      return Vec::new();
    }
    let Some(snapshot) = self.0.snapshot() else {
      return Vec::new();
    };
    let clean_set: FxHashSet<&str> = changed_clean_paths
      .iter()
      .map(|p| p.as_str().split(['?', '#']).next().unwrap_or(p.as_str()))
      .collect();
    let mut matched = Vec::new();
    let mut seen = FxHashSet::default();
    for module in &snapshot.module_table.modules {
      let id = module.id().as_arc_str();
      if id.as_str().ends_with("?__native_url_asset") {
        continue;
      }
      let clean = id.as_str().split(['?', '#']).next().unwrap_or(id.as_str());
      if clean_set.contains(clean) && seen.insert(id.clone()) {
        matched.push(id.clone());
      }
    }
    matched
  }

  pub fn codegen_cache_bytes(&self) -> usize {
    let finalized_bytes: usize = self
      .0
      .cached_finalized_modules
      .values()
      .map(|m| std::mem::size_of::<[u128; 2]>() + sources_bytes(&m.sources))
      .sum();
    let sourcemap_bytes: usize = self
      .0
      .cached_chunk_sourcemaps
      .values()
      .map(|m| {
        m.filename.len()
          + m.json.len()
          + m.post_minify_content.as_ref().map_or(0, |c| c.len())
          + m.pre_minify_placeholders.iter().map(|s| s.len()).sum::<usize>()
          + m.names.iter().map(|s| s.len()).sum::<usize>()
          + m.original_file_names.iter().map(|s| s.len()).sum::<usize>()
      })
      .sum();
    let multi_module_bytes: usize =
      self.0.cached_multi_module_chunks.values().map(multi_module_chunk_bytes).sum();
    finalized_bytes.saturating_add(sourcemap_bytes).saturating_add(multi_module_bytes)
  }

  pub fn modules_matching(&self, pattern: &str) -> Vec<ArcStr> {
    self.modules_matching_filter(pattern, |_| true)
  }

  pub fn modules_matching_filter(&self, pattern: &str, filter: impl Fn(&str) -> bool) -> Vec<ArcStr> {
    self
      .0
      .snapshot()
      .map(|snapshot| {
        snapshot
          .module_table
          .modules
          .iter()
          .filter_map(|module| {
            let id = module.id().as_str();
            ((id.ends_with(pattern) || id.starts_with(pattern)) && filter(id))
              .then(|| module.id().as_arc_str().clone())
          })
          .collect()
      })
      .unwrap_or_default()
  }

  pub fn modules_containing_source(&self, needle: &str) -> Vec<ArcStr> {
    use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
    if needle.is_empty() {
      return Vec::new();
    }
    if needle.starts_with("/r/")
      && let Some(snapshot) = self.0.snapshot()
    {
      return self
        .0
        .revision_url_modules
        .iter()
        .filter_map(|&idx| snapshot.module_table.modules.get(idx)?.as_normal())
        .filter(|module| module.ecma_view.source.contains(needle))
        .map(|module| module.id.as_arc_str().clone())
        .collect();
    }
    self
      .0
      .snapshot()
      .map(|snapshot| {
        snapshot
          .module_table
          .modules
          .par_iter()
          .filter_map(Module::as_normal)
          .filter(|module| module.ecma_view.source.contains(needle))
          .map(|module| module.id.as_arc_str().clone())
          .collect()
      })
      .unwrap_or_default()
  }

  pub fn asset_modules(&self) -> Vec<ArcStr> {
    self
      .0
      .snapshot()
      .map(|snapshot| {
        self
          .0
          .asset_emitter_modules
          .iter()
          .filter_map(|&idx| snapshot.module_table.modules.get(idx)?.as_normal())
          .map(|normal| normal.id.as_arc_str().clone())
          .collect()
      })
      .unwrap_or_default()
  }

  pub fn ast_used_bytes(&self) -> usize {
    self.0.ast_used_bytes()
  }

  pub fn ast_capacity_bytes(&self) -> usize {
    self.0.ast_capacity_bytes()
  }

  pub fn source_bytes(&self) -> usize {
    self.0.snapshot().map_or(0, |snapshot| snapshot.module_table.modules.iter()
      .filter_map(Module::as_normal).map(|module| module.ecma_view.source.len()).sum())
  }

  pub fn module_count(&self) -> usize {
    self.0.snapshot().map_or(0, |snapshot| snapshot.module_table.modules.len())
  }

  /// Evicts the least recently used entries of the codegen caches until each fits its budget.
  /// A build that misses an evicted entry renders it again, so this trades rebuild latency for
  /// memory and never changes output.
  pub fn trim_codegen_caches(&mut self, budget: CodegenCacheBudget) -> CodegenCacheTrim {
    self.0.trim_codegen_caches(budget)
  }

  /// Copies each AST whose arena has outgrown its contents into a right-sized arena, which drops
  /// the arena's spare capacity and the nodes that pre-processing discarded. Returns the arena
  /// capacity released.
  pub fn compact_asts(&mut self) -> usize {
    self.0.compact_asts()
  }
}

/// Byte budgets for [`RevisionScanCache::trim_codegen_caches`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodegenCacheBudget {
  pub finalized_module_bytes: usize,
  pub chunk_sourcemap_bytes: usize,
  /// Reusable chunk code: native chunks first, then multi-module chunks in what remains.
  pub native_chunk_bytes: usize,
}

/// What one [`RevisionScanCache::trim_codegen_caches`] evicted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodegenCacheTrim {
  pub finalized_modules: usize,
  pub chunk_sourcemaps: usize,
  pub native_chunks: usize,
  pub multi_module_chunks: usize,
  /// Estimated bytes no longer retained by the cache; shared allocations may live on elsewhere.
  pub bytes: usize,
}

/// When each codegen cache entry was last used, in [`ScanStageCache::codegen_epoch`]s.
#[derive(Debug, Default)]
pub struct CodegenLastUsed {
  finalized_modules: FxHashMap<ModuleIdx, u64>,
  chunk_sourcemaps: FxHashMap<[u128; 2], u64>,
  native_chunks: FxHashMap<ModuleIdx, u64>,
  multi_module_chunks: FxHashMap<[u128; 2], u64>,
}

impl CodegenLastUsed {
  fn clear(&mut self) {
    self.finalized_modules.clear();
    self.chunk_sourcemaps.clear();
    self.native_chunks.clear();
    self.multi_module_chunks.clear();
  }
}

/// Bytes per source map token: four line/column positions plus the source and name ids.
const SOURCEMAP_TOKEN_BYTES: usize = 24;

fn sources_bytes(sources: &CachedModuleSources) -> usize {
  sources.0.as_ref().map_or(0, |sources| {
    sources
      .iter()
      .map(|source| {
        source.content().len()
          + source.sourcemap().map_or(0, |map| {
            map.get_tokens().len() * SOURCEMAP_TOKEN_BYTES
              + map.get_sources().map(str::len).sum::<usize>()
              + map.get_names().map(str::len).sum::<usize>()
              + map.get_source_contents().map(|content| content.map_or(0, str::len)).sum::<usize>()
          })
      })
      .sum()
  })
}

fn chunk_sourcemap_bytes(map: &CachedChunkSourcemap) -> usize {
  map.filename.len()
    + map.json.len()
    + map.post_minify_content.as_ref().map_or(0, ArcStr::len)
    + map.pre_minify_placeholders.iter().map(String::len).sum::<usize>()
    + map.names.iter().map(String::len).sum::<usize>()
    + map.original_file_names.iter().map(String::len).sum::<usize>()
}

fn native_chunk_bytes(chunk: &CachedNativeChunk) -> usize {
  let map = chunk.sourcemap_asset.as_ref().map_or(0, |asset| asset.source.as_bytes().len());
  chunk.chunk.filename.len() + chunk.chunk.code.len() + map
}

fn multi_module_chunk_bytes(chunk: &CachedMultiModuleChunk) -> usize {
  let unshared = |code: &Arc<str>| if Arc::ptr_eq(code, &chunk.content) { 0 } else { code.len() };
  let finalized = chunk.finalized.as_ref().map_or(0, |finalized| {
    unshared(&finalized.code)
      + finalized.placeholders.iter().map(String::len).sum::<usize>()
      + finalized.content_hash.as_ref().map_or(0, |(_, hash)| hash.len())
  });
  chunk.content.len() + finalized
}

/// Stamps the entries of `entries` that `used` names, and any not stamped before, with `epoch`,
/// forgets stamps of entries that are gone, then evicts the oldest (the largest first among
/// equally old) until the entries' bytes fit `budget`. Returns the evicted count and bytes.
fn trim_least_recently_used<K: Copy + Eq + std::hash::Hash + Ord, V>(
  entries: &mut FxHashMap<K, V>,
  last_used: &mut FxHashMap<K, u64>,
  used: impl IntoIterator<Item = K>,
  epoch: u64,
  budget: usize,
  bytes_of: impl Fn(&V) -> usize,
) -> (usize, usize) {
  last_used.retain(|key, _| entries.contains_key(key));
  for key in used {
    if entries.contains_key(&key) {
      last_used.insert(key, epoch);
    }
  }
  let mut total = 0usize;
  let mut candidates = Vec::with_capacity(entries.len());
  for (key, value) in entries.iter() {
    let bytes = bytes_of(value);
    total = total.saturating_add(bytes);
    candidates.push((*last_used.entry(*key).or_insert(epoch), bytes, *key));
  }
  if total <= budget {
    return (0, 0);
  }
  candidates.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
  let (mut evicted, mut evicted_bytes) = (0, 0);
  for (_, bytes, key) in candidates {
    if total <= budget {
      break;
    }
    entries.remove(&key);
    last_used.remove(&key);
    total -= bytes;
    evicted += 1;
    evicted_bytes += bytes;
  }
  if evicted > 0 {
    entries.shrink_to_fit();
    last_used.shrink_to_fit();
  }
  (evicted, evicted_bytes)
}

/// A lazy entry's cached chunk: the scan generations of the modules that it ships and of their
/// imports, how its factories read the CommonJS modules that they import
/// ([`ScanStageCache::commonjs_reads`]), which it was rendered for, and its code, filename,
/// sourcemap filename and sourcemap.
pub type CachedLazyChunk = (
  Vec<(ModuleIdx, u64)>,
  Vec<(ModuleIdx, ModuleIdx, bool)>,
  String,
  String,
  Option<String>,
  Option<String>,
);

#[derive(Default, Debug)]
pub struct ScanStageCache {
  snapshot: Option<NormalizedScanStageOutput>,
  pub barrel_state: BarrelState,
  pub module_id_to_idx: FxHashMap<ModuleId, VisitState>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  /// Modules whose `importers` records were mutated by a partial scan.
  /// [`Self::merge`] re-derives their materialized importer sets, which the
  /// scan does only for re-scanned modules.
  pub modules_with_changed_importers: FxHashSet<ModuleIdx>,
  /// Files of an aborted (and reverted) partial scan; the next partial scan
  /// retries them, so their errors keep surfacing until the files are fixed.
  /// Only files the graph still needs are queued; see
  /// [`ModuleLoader::revert_partial_scan`](crate::module_loader::module_loader::ModuleLoader).
  /// Diagnostics of the deferred resolutions (`FAILURE_MODULE_PREFIX` ids) whose synthetic
  /// modules no scan has spawned yet. A lazy barrel's `tracked_records` keep their ids across
  /// scans, so the diagnostic must outlive the scan that resolved them (#700).
  pub deferred_resolution_diagnostics:
    FxHashMap<ModuleId, crate::module_loader::task_context::DeferredResolutionDiagnostic>,
  pub pending_rescans: Vec<ResolvedId>,
  /// Whether the last scan was a partial one that failed only in modules that the graph it leaves
  /// no longer reaches (`ModuleLoader::unreached`): in their tasks, or in the parse or link
  /// failures that the scan finds once it loaded every module. It failed all the same, but a
  /// fresh build of the revision never loads them, so a caller may build the revision anew rather
  /// than report the failure (#690).
  pub last_scan_failures_unreached: bool,
  pub user_defined_entry: FxHashSet<ModuleId>,
  // Usage: Map file path emitted by watcher to corresponding module index
  pub module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
  // Usage: Map module stable id injected to client code to corresponding module index
  pub module_idx_by_stable_id: FxHashMap<StableModuleId, ModuleIdx>,
  pub native_cell_modules: FxHashSet<ModuleIdx>,
  pub export_star_modules: FxHashSet<ModuleIdx>,
  /// Whether a scan of this cache ever held a module through whose records a page may load a
  /// record that it never prunes: one that names a dependency in an `import.meta.hot.accept()`
  /// call, whose record the page loads with no edge to it, one with an `import()`, which a
  /// loader that the page saved can call after an update dropped the import, past the roots of
  /// the page's first patch, or one with an `import defer`, whose target's record a read of the
  /// namespace that the page saved runs as late. Such a page may hold a module's record after an
  /// update retires it, which a page otherwise prunes (#732). It never resets, not even for a
  /// full scan: a page may hold records of any earlier revision.
  pub late_records: bool,
  pub revision_url_modules: FxHashSet<ModuleIdx>,
  pub asset_emitter_modules: FxHashSet<ModuleIdx>,
  pub next_scan_generation: u64,
  pub module_scan_generations: IndexVec<ModuleIdx, u64>,
  pub cached_chunk_namings: FxHashMap<ModuleIdx, CachedChunkNaming>,
  pub naming_order_signature: Option<[u128; 2]>,
  pub cached_native_chunks: FxHashMap<ModuleIdx, CachedNativeChunk>,
  pub cached_finalized_modules: FxHashMap<ModuleIdx, CachedFinalizedModule>,
  pub current_rendered_sources: FxHashMap<ModuleIdx, CachedModuleSources>,
  pub current_module_finalize_digests: FxHashMap<ModuleIdx, [u128; 2]>,
  pub current_module_was_hit: FxHashSet<ModuleIdx>,
  pub cached_chunk_sourcemaps: FxHashMap<[u128; 2], CachedChunkSourcemap>,
  pub current_chunk_map_digests: FxHashMap<rolldown_common::ChunkIdx, [u128; 2]>,
  pub current_chunk_minify_state:
    FxHashMap<rolldown_common::ChunkIdx, ([u8; 32], Vec<String>, Option<ArcStr>)>,
  pub cached_multi_module_chunks: FxHashMap<[u128; 2], CachedMultiModuleChunk>,
  /// The [`Self::cached_multi_module_chunks`] keys that the last generate rendered.
  pub current_multi_module_chunk_keys: FxHashSet<[u128; 2]>,
  pub sourcemap_content_filter: Option<SourcemapContentFilterHolder>,
  pub pre_redacted_maps: FxHashSet<ArcStr>,
  /// The lazy chunk of each lazy entry, by module id ([`CachedLazyChunk`]).
  pub cached_lazy_chunks: FxHashMap<String, CachedLazyChunk>,
  /// How the factories that patches and lazy chunks shipped since the session last cleared it
  /// read each CommonJS module that they import, by importer and importee: whether through
  /// `__toESM`, as the link of the scan that they rendered against decided (#748). A later scan
  /// of the same update can decide otherwise, as one that adds an importer of a module's default
  /// (`HmrStage::changed_commonjs_read`).
  pub commonjs_reads: FxHashSet<(ModuleIdx, ModuleIdx, bool)>,
  pub runtime_rendered_chunk: Option<Arc<RollupRenderedChunk>>,
  pub omit_reused_chunk_bytes: bool,
  /// Keeps each module's AST and scope tree in the arenas that its scan built, and the capacity
  /// that linking leaves in the cached link output, instead of right-sizing them
  /// ([`crate::utils::retained_memory`]). Tests compare the two.
  pub keep_unused_capacity: bool,
  pub last_reused_chunk_filenames: FxHashSet<ArcStr>,
  pub last_fast_native_relink: bool,
  /// Why the last generation linked the whole graph rather than relinking what the update
  /// changed: the label of the check that rejected the fast native relink, or of what kept the
  /// generation from trying one.
  pub last_native_relink_fallback: Option<&'static str>,
  /// What that check rejected, such as the modules of an import, when it names them.
  pub last_native_relink_fallback_detail: Option<String>,
  pub last_rebuilt_chunk_filenames: Vec<ArcStr>,
  pub last_rebuilt_chunk_outputs: Vec<rolldown_common::Output>,
  pub cached_native_relink: Option<Box<CachedNativeRelinkState>>,
  /// The constants that the last link inlined for the CommonJS exports of each module
  /// (`link_stage::inlined_commonjs_constants`), which a page that loaded its output holds (#693).
  pub inlined_commonjs_constants:
    Option<std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>>,
  /// Whether the last link split the default object of each JSON module that it reaches into the
  /// bindings of its properties (`link_stage::split_json_defaults`), which decides how the records
  /// of a page that loaded its output read the properties (#660).
  pub split_json_defaults: Option<std::collections::BTreeMap<String, bool>>,
  /// Whether the records of the last link's output evaluate each wrapped CommonJS module as they
  /// evaluate, or leave it to its first `require` (`render_chunk_exports::commonjs_evaluations`),
  /// which a page that loaded the output did once (#660).
  pub commonjs_evaluations: Option<std::collections::BTreeMap<String, bool>>,
  /// How the records of the last link's output convert the exports of each CommonJS module that
  /// ES modules import (`link_stage::commonjs_interop`), as the records of a page that loaded the
  /// output keep doing (#660).
  pub commonjs_interop: Option<std::collections::BTreeMap<String, CommonjsInterop>>,
  pub dirty_native_modules: FxHashSet<ModuleIdx>,
  /// Modules that partial scans appended while a native relink state was cached, which a
  /// fast native relink inserts into its cached link output.
  pub added_native_modules: Vec<ModuleIdx>,
  /// Cached modules whose importer sets partial scans changed while a native relink state
  /// was cached; a fast native relink checks that their link metadata stays valid.
  pub native_importer_changes: FxHashSet<ModuleIdx>,
  pub last_native_graph_change: Option<NativeGraphChange>,
  /// Counts [`Self::trim_codegen_caches`] calls, which stamp the codegen cache entries a build used.
  pub codegen_epoch: u64,
  pub codegen_last_used: CodegenLastUsed,
}

impl ScanStageCache {
  pub fn reset_for_full_scan(&mut self) {
    let omit_reused_chunk_bytes = self.omit_reused_chunk_bytes;
    let keep_unused_capacity = self.keep_unused_capacity;
    let sourcemap_content_filter = self.sourcemap_content_filter.take();
    // Pages that loaded an earlier revision's records keep them past a full scan.
    let late_records = self.late_records;
    *self = Self {
      omit_reused_chunk_bytes,
      keep_unused_capacity,
      sourcemap_content_filter,
      late_records,
      ..Self::default()
    };
  }

  pub fn ast_used_bytes(&self) -> usize {
    self.snapshot().map_or(0, |snapshot| {
      snapshot
        .index_ecma_ast
        .iter()
        .flatten()
        .map(|ast| ast.program.borrow_owner().allocator.used_bytes())
        .sum()
    })
  }

  pub fn ast_capacity_bytes(&self) -> usize {
    self.snapshot().map_or(0, |snapshot| {
      snapshot
        .index_ecma_ast
        .iter()
        .flatten()
        .map(|ast| ast.program.borrow_owner().allocator.capacity())
        .sum()
    })
  }

  pub fn has_valid_native_relink(&self) -> bool {
    self.snapshot.is_some() && self.cached_native_relink.is_some()
  }

  pub fn invalidate_native_relink(&mut self) {
    let dirty = std::mem::take(&mut self.dirty_native_modules);
    // A failed fast relink may have seeded an appended module's link data without its scope
    // tree; the snapshot still holds the scanned one.
    let added = std::mem::take(&mut self.added_native_modules);
    self.native_importer_changes.clear();
    let Some(mut cached) = self.cached_native_relink.take() else {
      return;
    };
    if let Some(snapshot) = self.snapshot.as_mut() {
      let modules_len = snapshot.module_table.modules.len();
      for (idx, orig_db) in std::mem::take(&mut cached.link_output.pre_lazy_export_symbols) {
        if idx.index() < cached.link_output.symbol_db.inner().len() {
          *cached.link_output.symbol_db.local_db_mut(idx) = orig_db;
        }
      }
      let symbol_db = std::mem::take(&mut cached.link_output.symbol_db);
      for (idx, symbol_ref_db) in symbol_db.into_inner().into_iter_enumerated() {
        if dirty.contains(&idx) || added.contains(&idx) || idx.index() >= modules_len {
          continue;
        }
        let Some(db_for_module) = symbol_ref_db else {
          continue;
        };
        let cache_db = snapshot.symbol_ref_db.local_db_mut(idx);
        cache_db.merge_from_build(db_for_module);
      }
    }
    crate::utils::defer_drop::spawn_drop(cached);
  }

  pub(crate) fn contains_leaf(&self, id: &str) -> bool {
    let Some(snapshot) = self.snapshot() else {
      return false;
    };
    if let Some(v) = self.module_id_to_idx.get(id)
      && let Some(module) = snapshot.module_table.modules.get(v.idx()).and_then(Module::as_normal)
    {
      return module.import_records.is_empty();
    }
    snapshot
      .module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .any(|module| module.id.as_str() == id && module.import_records.is_empty())
  }

  pub(crate) fn can_partial_rescan(&self, id: &str) -> bool {
    let Some(snapshot) = self.snapshot() else {
      return false;
    };
    if let Some(v) = self.module_id_to_idx.get(id)
      && let Some(module) = snapshot.module_table.modules.get(v.idx()).and_then(Module::as_normal)
    {
      return !self.barrel_state.barrel_infos.get(&module.idx).is_some_and(Option::is_some);
    }
    snapshot.module_table.modules.iter().filter_map(Module::as_normal).any(|module| {
      module.id.as_str() == id
        && !self.barrel_state.barrel_infos.get(&module.idx).is_some_and(Option::is_some)
    })
  }

  pub(crate) fn module_import_signature(&self, id: &str) -> Option<ModuleImportSignature> {
    let snapshot = self.snapshot()?;
    let module = if let Some(v) = self.module_id_to_idx.get(id)
      && let Some(m) = snapshot.module_table.modules.get(v.idx()).and_then(Module::as_normal)
    {
      m
    } else {
      snapshot
        .module_table
        .modules
        .iter()
        .filter_map(Module::as_normal)
        .find(|module| module.id.as_str() == id)?
    };
    if self.barrel_state.barrel_infos.get(&module.idx).is_some_and(Option::is_some) {
      return None;
    }
    let records =
      module.import_records.iter().map(|r| (r.resolved_module, r.kind, r.meta.bits())).collect();
    let mut named_imports: Vec<_> = module
      .named_imports
      .values()
      .map(|ni| (ni.record_idx, ni.imported.clone()))
      .collect();
    named_imports.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| format!("{:?}", a.1).cmp(&format!("{:?}", b.1))));
    Some((records, named_imports))
  }

  pub fn module_scan_generation(&self, idx: ModuleIdx) -> u64 {
    self.module_scan_generations.get(idx).copied().unwrap_or(0)
  }

  /// See [`RevisionScanCache::trim_codegen_caches`]. Also drops the previous build's per-build
  /// render state, which the next generate rebuilds before reading and which would otherwise keep
  /// evicted sources alive.
  pub fn trim_codegen_caches(&mut self, budget: CodegenCacheBudget) -> CodegenCacheTrim {
    self.codegen_epoch += 1;
    let epoch = self.codegen_epoch;
    let (finalized_modules, finalized_bytes) = trim_least_recently_used(
      &mut self.cached_finalized_modules,
      &mut self.codegen_last_used.finalized_modules,
      self.current_module_finalize_digests.keys().copied(),
      epoch,
      budget.finalized_module_bytes,
      |module| std::mem::size_of::<[u128; 2]>() + sources_bytes(&module.sources),
    );
    let (chunk_sourcemaps, sourcemap_bytes) = trim_least_recently_used(
      &mut self.cached_chunk_sourcemaps,
      &mut self.codegen_last_used.chunk_sourcemaps,
      self.current_chunk_map_digests.values().copied(),
      epoch,
      budget.chunk_sourcemap_bytes,
      chunk_sourcemap_bytes,
    );
    let reused: Vec<ModuleIdx> = self
      .cached_native_chunks
      .iter()
      .filter(|(_, cached)| self.last_reused_chunk_filenames.contains(&cached.chunk.filename))
      .map(|(idx, _)| *idx)
      .collect();
    let (native_chunks, native_bytes) = trim_least_recently_used(
      &mut self.cached_native_chunks,
      &mut self.codegen_last_used.native_chunks,
      reused,
      epoch,
      budget.native_chunk_bytes,
      native_chunk_bytes,
    );
    let retained_native_bytes: usize =
      self.cached_native_chunks.values().map(native_chunk_bytes).sum();
    let (multi_module_chunks, multi_module_bytes) = trim_least_recently_used(
      &mut self.cached_multi_module_chunks,
      &mut self.codegen_last_used.multi_module_chunks,
      self.current_multi_module_chunk_keys.iter().copied(),
      epoch,
      budget.native_chunk_bytes.saturating_sub(retained_native_bytes),
      multi_module_chunk_bytes,
    );
    self.current_rendered_sources = FxHashMap::default();
    self.current_module_finalize_digests = FxHashMap::default();
    self.current_module_was_hit = FxHashSet::default();
    self.current_chunk_map_digests = FxHashMap::default();
    self.current_chunk_minify_state = FxHashMap::default();
    self.current_multi_module_chunk_keys = FxHashSet::default();
    CodegenCacheTrim {
      finalized_modules,
      chunk_sourcemaps,
      native_chunks,
      multi_module_chunks,
      bytes: finalized_bytes + sourcemap_bytes + native_bytes + multi_module_bytes,
    }
  }

  /// See [`RevisionScanCache::compact_asts`].
  pub fn compact_asts(&mut self) -> usize {
    use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
    /// Arena slack below this is not worth a copy.
    const MIN_SLACK: usize = 4 * 1024;
    let Some(snapshot) = self.snapshot.as_mut() else {
      return 0;
    };
    snapshot
      .index_ecma_ast
      .raw
      .par_iter_mut()
      .filter_map(|slot| {
        let ast = slot.as_mut()?;
        let allocator = &ast.program.borrow_owner().allocator;
        let (used, capacity) = (allocator.used_bytes(), allocator.capacity());
        let slack = capacity.saturating_sub(used);
        if slack < MIN_SLACK || slack < used / 8 {
          return None;
        }
        let released = crate::utils::retained_memory::compact_ast(ast);
        (released > 0).then_some(released)
      })
      .sum()
  }

  #[inline]
  pub fn set_snapshot(&mut self, cache: NormalizedScanStageOutput) {
    self.build_module_index_maps(&cache);
    self.next_scan_generation = self.next_scan_generation.wrapping_add(1).max(1);
    let generation = self.next_scan_generation;
    self.module_scan_generations =
      IndexVec::from_vec(vec![generation; cache.module_table.modules.len()]);
    self.cached_chunk_namings.clear();
    self.naming_order_signature = None;
    self.cached_native_chunks.clear();
    self.cached_finalized_modules.clear();
    self.current_rendered_sources.clear();
    self.current_module_finalize_digests.clear();
    self.current_module_was_hit.clear();
    self.cached_chunk_sourcemaps.clear();
    self.current_chunk_map_digests.clear();
    self.cached_multi_module_chunks.clear();
    self.current_multi_module_chunk_keys.clear();
    self.pre_redacted_maps.clear();
    self.cached_lazy_chunks.clear();
    self.runtime_rendered_chunk = None;
    self.last_reused_chunk_filenames.clear();
    self.last_fast_native_relink = false;
    self.last_native_relink_fallback = None;
    self.last_rebuilt_chunk_filenames.clear();
    self.last_rebuilt_chunk_outputs.clear();
    self.cached_native_relink = None;
    self.dirty_native_modules.clear();
    self.added_native_modules.clear();
    self.native_importer_changes.clear();
    self.last_native_graph_change = None;
    self.codegen_last_used.clear();
    self.snapshot = Some(cache);
  }

  /// # Panic
  /// - if the snapshot is unset
  pub fn get_snapshot_mut(&mut self) -> &mut NormalizedScanStageOutput {
    self.snapshot.as_mut().unwrap()
  }

  /// Useful when workarounding rustc borrow rules
  pub fn take_snapshot(&mut self) -> Option<NormalizedScanStageOutput> {
    self.snapshot.take()
  }

  pub async fn update_defer_sync_data(&mut self, options: &SharedOptions) -> BuildResult<()> {
    if options.defer_sync_scan_data.is_none() {
      return Ok(());
    }
    self.invalidate_native_relink();
    if let Some(mut snapshot) = self.take_snapshot() {
      // `defer_sync_scan_data` mutates `snapshot` in place; restore it on every
      // outcome. Bailing with `?` would drop it, leaving `self.snapshot == None`
      // and panicking the next HMR cycle's `get_snapshot()`. A partially-synced
      // snapshot is recoverable; a missing one is not.
      // See internal-docs/bundler-data-lifecycle/implementation.md ("Cache integrity on a failed build").
      let result = defer_sync_scan_data(options, &self.module_id_to_idx, &mut snapshot).await;
      self.next_scan_generation = self.next_scan_generation.wrapping_add(1).max(1);
      let generation = self.next_scan_generation;
      self.module_scan_generations =
        IndexVec::from_vec(vec![generation; snapshot.module_table.modules.len()]);
      self.cached_native_chunks.clear();
      self.cached_finalized_modules.clear();
      self.current_rendered_sources.clear();
      self.cached_multi_module_chunks.clear();
      self.cached_lazy_chunks.clear();
      self.snapshot = Some(snapshot);
      result?;
    }
    Ok(())
  }

  /// # Panic
  /// - if the snapshot is unset
  pub fn get_snapshot(&self) -> &NormalizedScanStageOutput {
    self.snapshot.as_ref().unwrap()
  }

  /// Non-panicking variant of [`Self::get_snapshot`].
  pub fn snapshot(&self) -> Option<&NormalizedScanStageOutput> {
    self.snapshot.as_ref()
  }

  /// Between builds the cache is either empty (no snapshot: only a full scan
  /// is possible) or a valid graph any partial scan can build on. A failed
  /// partial scan keeps the latter true by reverting its mutations
  /// (`ModuleLoader::revert_partial_scan`).
  pub fn has_snapshot(&self) -> bool {
    self.snapshot.is_some()
  }

  /// Re-derives the `importers` edge list (one record per resolved import
  /// record, keyed by the imported module) from the snapshot. Restores the
  /// pre-scan list up to slot order; consumers treat slots as sets.
  ///
  /// # Panic
  /// - if the snapshot is unset
  pub fn derive_importers_from_snapshot(&self) -> IndexVec<ModuleIdx, Vec<ImporterRecord>> {
    let snapshot = self.get_snapshot();
    let mut importers = IndexVec::from_vec(
      std::iter::repeat_with(Vec::new).take(snapshot.module_table.modules.len()).collect(),
    );
    for module in &snapshot.module_table.modules {
      let Some(module) = module.as_normal() else {
        continue;
      };
      for record in &module.ecma_view.import_records {
        if let Some(dep_idx) = record.resolved_module {
          importers[dep_idx].push(ImporterRecord {
            importer_path: module.id.clone(),
            importer_idx: module.idx,
            kind: record.kind,
          });
        }
      }
    }
    importers
  }

  pub fn merge(
    &mut self,
    mut scan_stage_output: ScanStageOutput,
    plugin_driver: &PluginDriver,
  ) -> BuildResult<()> {
    fn module_has_tla(module: &Module) -> bool {
      module.as_normal().is_some_and(|normal_module| {
        normal_module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait)
      })
    }

    let Some(ref mut cache) = self.snapshot else {
      self.last_fast_native_relink = false;
      self.last_native_relink_fallback = None;
      self.last_rebuilt_chunk_filenames.clear();
      self.last_rebuilt_chunk_outputs.clear();
      self.cached_native_relink = None;
      self.dirty_native_modules.clear();
      self.added_native_modules.clear();
      self.native_importer_changes.clear();
      self.last_native_graph_change = None;
      let snapshot: NormalizedScanStageOutput =
        scan_stage_output.try_into().map_err(|e: &'static str| vec![anyhow::anyhow!(e).into()])?;
      self.build_module_index_maps(&snapshot);
      self.snapshot = Some(snapshot);
      return Ok(());
    };
    self.last_fast_native_relink = false;
    self.last_native_relink_fallback = None;
    self.last_rebuilt_chunk_filenames.clear();
    self.last_rebuilt_chunk_outputs.clear();
    self.last_native_graph_change = None;
    let modules = match scan_stage_output.module_table {
      rolldown_common::HybridIndexVec::IndexVec(_index_vec) => {
        unreachable!()
      }
      rolldown_common::HybridIndexVec::Map(map) => {
        if map.is_empty()
          && self.modules_with_changed_importers.is_empty()
          && scan_stage_output.entry_points.is_empty()
          && self.barrel_state.resolved_barrel_modules.is_empty()
          && scan_stage_output.dynamic_import_exports_usage_map.is_empty()
        {
          return Ok(());
        }
        let mut modules = map.into_iter().collect_vec();
        modules.sort_unstable_by_key(|(k, _)| *k);
        modules
      }
    };
    let rescanned_module_idxs = modules.iter().map(|(idx, _)| *idx).collect::<FxHashSet<_>>();
    // A fast native relink inserts appended modules at the end of its cached tables, so they
    // must follow the cached modules contiguously.
    let cached_modules_len = cache.module_table.modules.len();
    let mut next_appended = cached_modules_len;
    let appended_contiguously = modules.iter().all(|(new_idx, _)| {
      if new_idx.index() < cached_modules_len {
        return true;
      }
      let contiguous = new_idx.index() == next_appended;
      next_appended += 1;
      contiguous
    });
    let mut invalidate_relink = !scan_stage_output.entry_points.is_empty()
      || !self.barrel_state.resolved_barrel_modules.is_empty()
      || !scan_stage_output.dynamic_import_exports_usage_map.is_empty()
      || !scan_stage_output.overrode_preserve_entry_signature_map.is_empty()
      || !scan_stage_output.entry_point_to_reference_ids.is_empty()
      || !appended_contiguously
      || modules.iter().any(|(new_idx, new_module)| {
        !new_module.is_normal()
          || self.module_id_to_idx.get(new_module.id()).map(|v| v.idx()) != Some(*new_idx)
      })
      || cache.entry_points.iter().any(|ep| {
        ep.kind.is_dynamic_import()
          && ep
            .related_stmt_infos
            .iter()
            .any(|(importer_idx, _, _, _)| rescanned_module_idxs.contains(importer_idx))
      });
    self.next_scan_generation = self.next_scan_generation.wrapping_add(1).max(1);
    let merge_generation = self.next_scan_generation;
    if self.module_scan_generations.len() < cache.module_table.modules.len() {
      self.module_scan_generations.resize(cache.module_table.modules.len(), merge_generation);
    }
    fn module_has_export_star(module: &Module) -> bool {
      module.as_normal().is_some_and(|normal| {
        normal.import_records.iter().any(|rec| {
          rec.kind == rolldown_common::ImportKind::Import
            && rec.meta.contains(rolldown_common::ImportRecordMeta::IsExportStar)
        })
      })
    }
    fn module_loads_late_records(module: &Module) -> bool {
      module.as_normal().is_some_and(|normal| {
        normal.import_records.iter().any(|rec| {
          matches!(
            rec.kind,
            rolldown_common::ImportKind::HotAccept | rolldown_common::ImportKind::DynamicImport
          )
        })
      })
    }
    // An `import defer` declaration links its target's record without evaluating it, which a
    // namespace read that the page saved evaluates later, also after an update dropped it.
    fn defers_imports(ast: Option<&rolldown_ecmascript::EcmaAst>) -> bool {
      ast.is_some_and(|ast| {
        ast.program().body.iter().any(|statement| {
          matches!(
            statement,
            oxc::ast::ast::Statement::ImportDeclaration(import)
              if import.phase == Some(oxc::ast::ast::ImportPhase::Defer)
          )
        })
      })
    }
    fn module_is_native_cell(module: &Module) -> bool {
      module
        .as_normal()
        .is_some_and(|normal| crate::utils::native_records::is_native_cell(normal.id.as_str()))
    }
    fn module_has_revision_url(module: &Module) -> bool {
      module.as_normal().is_some_and(|normal| {
        let mut rest = normal.ecma_view.source.as_str();
        while let Some(pos) = rest.find("/r/") {
          rest = &rest[pos + 3..];
          if rest.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            return true;
          }
        }
        false
      })
    }
    fn module_is_asset_emitter(module: &Module) -> bool {
      module.as_normal().is_some_and(|normal| {
        (!normal.ecma_view.rolldown_file_url_references.is_empty()
          && !normal.id.as_str().ends_with("?__native_url_asset"))
          || normal.import_records.iter().any(|r| r.kind == rolldown_common::ImportKind::NewUrl)
      })
    }

    // merge module_table, index_ast_scope, index_ecma_ast
    let mut index_maps_need_rebuild = false;
    for (new_idx, new_module) in modules {
      let idx = self.module_id_to_idx[new_module.id()].idx();

      let incoming_tla_span = scan_stage_output.tla_keyword_span_map.get(&new_idx).copied();

      if new_idx.index() >= cache.module_table.modules.len() {
        let new_module_idx = ModuleIdx::from_usize(cache.module_table.modules.len());

        if let rolldown_common::Module::Normal(normal_module) = &new_module {
          self
            .module_idx_by_abs_path
            .insert(ArcStr::from(normal_module.id.as_arc_str().to_slash()), new_module_idx);
        }
        self.module_idx_by_stable_id.insert(new_module.stable_id().clone(), new_module_idx);

        if module_has_tla(&new_module) {
          cache.tla_module_count += 1;
        }
        if module_is_native_cell(&new_module) {
          self.native_cell_modules.insert(new_module_idx);
        } else {
          self.native_cell_modules.remove(&new_module_idx);
        }
        if module_has_export_star(&new_module) {
          self.export_star_modules.insert(new_module_idx);
        } else {
          self.export_star_modules.remove(&new_module_idx);
        }
        self.late_records |= module_loads_late_records(&new_module)
          || defers_imports(scan_stage_output.index_ecma_ast.try_get(new_idx).and_then(Option::as_ref));
        if module_has_revision_url(&new_module) {
          self.revision_url_modules.insert(new_module_idx);
        } else {
          self.revision_url_modules.remove(&new_module_idx);
        }
        if module_is_asset_emitter(&new_module) {
          self.asset_emitter_modules.insert(new_module_idx);
        } else {
          self.asset_emitter_modules.remove(&new_module_idx);
        }
        if let Some(span) = incoming_tla_span {
          cache.tla_keyword_span_map.insert(new_module_idx, span);
        }
        cache.symbol_ref_db.store_local_db(
          new_module_idx,
          std::mem::take(scan_stage_output.symbol_ref_db.local_db_mut(new_idx)),
        );
        cache.module_table.modules.push(new_module);
        cache.index_ecma_ast.push(scan_stage_output.index_ecma_ast.get_mut(new_idx).take());
        cache.stmt_infos.push(std::mem::replace(
          scan_stage_output.stmt_infos.get_mut(new_idx),
          rolldown_common::StmtInfos::new(),
        ));
        self.module_scan_generations.push(merge_generation);
        if self.cached_native_relink.is_some() {
          self.added_native_modules.push(new_module_idx);
        }
        continue;
      }
      {
        let old_module = &cache.module_table[idx];
        let same_stable = old_module.stable_id() == new_module.stable_id()
          && self.module_idx_by_stable_id.get(new_module.stable_id()) == Some(&idx);
        let same_normal_path = match (old_module.as_normal(), new_module.as_normal()) {
          (Some(old_n), Some(new_n)) => {
            old_n.id == new_n.id
              && self.module_idx_by_abs_path.get(new_n.id.as_arc_str().to_slash().as_ref())
                == Some(&idx)
          }
          (None, None) => true,
          _ => false,
        };
        if !same_stable || !same_normal_path {
          index_maps_need_rebuild = true;
        }
      }
      if module_is_native_cell(&new_module) {
        self.native_cell_modules.insert(idx);
      } else {
        self.native_cell_modules.remove(&idx);
      }
      if module_has_export_star(&new_module) {
        self.export_star_modules.insert(idx);
      } else {
        self.export_star_modules.remove(&idx);
      }
      self.late_records |= module_loads_late_records(&new_module)
        || defers_imports(scan_stage_output.index_ecma_ast.try_get(new_idx).and_then(Option::as_ref));
      if module_has_revision_url(&new_module) {
        self.revision_url_modules.insert(idx);
      } else {
        self.revision_url_modules.remove(&idx);
      }
      if module_is_asset_emitter(&new_module) {
        self.asset_emitter_modules.insert(idx);
      } else {
        self.asset_emitter_modules.remove(&idx);
      }
      self.module_scan_generations[idx] = merge_generation;
      self.cached_native_chunks.remove(&idx);
      self.cached_finalized_modules.remove(&idx);
      if self.cached_native_relink.is_some() {
        self.dirty_native_modules.insert(idx);
      }
      let old_has_tla = module_has_tla(&cache.module_table[idx]);
      let new_has_tla = module_has_tla(&new_module);
      if old_has_tla && !new_has_tla {
        debug_assert!(
          cache.tla_module_count > 0,
          "tla_module_count underflow: decrement called when count is already 0"
        );
        cache.tla_module_count -= 1;
      } else if !old_has_tla && new_has_tla {
        cache.tla_module_count += 1;
      }
      match incoming_tla_span {
        Some(span) => {
          cache.tla_keyword_span_map.insert(idx, span);
        }
        None => {
          cache.tla_keyword_span_map.remove(&idx);
        }
      }
      cache.module_table[idx] = new_module;
      cache.index_ecma_ast[idx] = scan_stage_output.index_ecma_ast.get_mut(new_idx).take();
      cache.stmt_infos[idx] = std::mem::replace(
        scan_stage_output.stmt_infos.get_mut(new_idx),
        rolldown_common::StmtInfos::new(),
      );
      std::mem::swap(
        cache.symbol_ref_db.local_db_mut(idx),
        scan_stage_output.symbol_ref_db.local_db_mut(new_idx),
      );
    }

    // The scan rebuilds the materialized importer sets only for the modules
    // it re-scanned. Re-derive them for cached modules whose incoming edges
    // changed. See internal-docs/cache/implementation.md.
    let modules_with_changed_importers = std::mem::take(&mut self.modules_with_changed_importers);
    let mut importers_actually_changed = Vec::new();
    for idx in &modules_with_changed_importers {
      // The idx may belong to a module from a scan that failed before merging.
      let Some(module) = cache.module_table.modules.get_mut(*idx).and_then(Module::as_normal_mut)
      else {
        continue;
      };
      if self.cached_native_relink.is_some() && !invalidate_relink {
        let prev_importers = module.ecma_view.importers.clone();
        let prev_importers_idx = module.ecma_view.importers_idx.clone();
        let prev_dynamic_importers = module.ecma_view.dynamic_importers.clone();
        module.ecma_view.rebuild_importer_sets(&self.importers[*idx]);
        if module.ecma_view.importers != prev_importers
          || module.ecma_view.importers_idx != prev_importers_idx
          || module.ecma_view.dynamic_importers != prev_dynamic_importers
        {
          importers_actually_changed.push(*idx);
        }
      } else {
        module.ecma_view.rebuild_importer_sets(&self.importers[*idx]);
        importers_actually_changed.push(*idx);
      }
    }
    if !importers_actually_changed.is_empty() {
      // Edges to or from appended and retired modules change importer sets; a fast native
      // relink checks those modules instead of discarding its cached link output.
      if self.cached_native_relink.is_some() && !invalidate_relink {
        self.native_importer_changes.extend(importers_actually_changed.iter().copied());
      } else {
        invalidate_relink = true;
      }
    }

    // Deleted or retargeted imports have no incoming row, so clean every cached dynamic entry.
    // See internal-docs/cache/implementation.md (`ScanStageCache::merge` — the write path).
    cache.entry_points.retain_mut(|entry_point| {
      if !entry_point.kind.is_dynamic_import() {
        return true;
      }
      entry_point
        .related_stmt_infos
        .retain(|(importer_idx, _, _, _)| !rescanned_module_idxs.contains(importer_idx));
      !entry_point.related_stmt_infos.is_empty()
    });

    for entry_point in scan_stage_output.entry_points {
      if let Some(old_entry_point) = cache
        .entry_points
        .iter_mut()
        .find(|old_entry| old_entry.kind == entry_point.kind && old_entry.idx == entry_point.idx)
      {
        old_entry_point.related_stmt_infos.extend(entry_point.related_stmt_infos);
      } else {
        cache.entry_points.push(entry_point);
      }
    }

    // Update barrel module resolved import records
    let resolved_barrel_modules = std::mem::take(&mut self.barrel_state.resolved_barrel_modules);
    for (barrel_module_idx, resolved_imports) in resolved_barrel_modules {
      let barrel_module = &mut cache.module_table[barrel_module_idx];
      if let Module::Normal(normal_module) = barrel_module {
        resolved_imports.into_iter().for_each(|(rec_idx, new_idx)| {
          normal_module.import_records[rec_idx].resolved_module = Some(new_idx);
        });
      }
    }

    // Recompute user-defined entry modules for this build instead of monotonically extending.
    // `scan_stage_output.user_defined_entry_modules` only contains entries discovered in the
    // current scan (e.g. changed modules + emitted entries), so we additionally keep configured
    // root entries that remain valid in cache.
    let mut user_defined_entry_modules = scan_stage_output.user_defined_entry_modules;
    for user_defined_entry_id in &self.user_defined_entry {
      let Some(visit_state) = self.module_id_to_idx.get(user_defined_entry_id) else {
        continue;
      };
      let idx = visit_state.idx();
      if cache.module_table.modules.get(idx).is_some() {
        user_defined_entry_modules.insert(idx);
      }
    }
    // Entries emitted via `emitFile(type: 'chunk')` may not be re-discovered by a
    // partial scan. The HMR stage's partial scans bypass `ScanStage`, so `buildStart`
    // never runs there. `ScanStage` partial scans do run it, but a plugin may emit
    // from `resolveId`/`load`/`transform`/`moduleParsed`, and those only re-run for
    // re-scanned modules. Their rows persist in `entry_points`, so keep their
    // modules flagged as entries too.
    for entry in &cache.entry_points {
      if matches!(entry.kind, rolldown_common::EntryPointKind::EmittedUserDefined) {
        user_defined_entry_modules.insert(entry.idx);
      }
    }
    cache.user_defined_entry_modules = user_defined_entry_modules;

    // Keep the plugin-facing `ModuleInfo.importers` of those modules in
    // sync as well; the scan refreshes it only for re-scanned modules.
    for idx in importers_actually_changed {
      let Some(module) = cache.module_table.modules.get(idx).and_then(Module::as_normal) else {
        continue;
      };
      plugin_driver.set_module_info(
        &module.id,
        Arc::new(module.to_module_info(None, cache.user_defined_entry_modules.contains(&idx))),
      );
    }
    if index_maps_need_rebuild && let Some(snapshot) = self.snapshot.take() {
      self.build_module_index_maps(&snapshot);
      self.snapshot = Some(snapshot);
    }
    if invalidate_relink || index_maps_need_rebuild {
      self.invalidate_native_relink();
    }

    Ok(())
  }

  fn build_module_index_maps(&mut self, build_snapshot: &NormalizedScanStageOutput) {
    self.module_idx_by_abs_path.clear();
    self.module_idx_by_stable_id.clear();
    self.native_cell_modules.clear();
    self.export_star_modules.clear();
    self.revision_url_modules.clear();
    self.asset_emitter_modules.clear();

    for module in &build_snapshot.module_table.modules {
      if let rolldown_common::Module::Normal(normal_module) = module {
        let filename = ArcStr::from(normal_module.id.as_arc_str().to_slash());
        let module_idx = normal_module.idx;
        self.module_idx_by_abs_path.insert(filename, module_idx);
        if crate::utils::native_records::is_native_cell(normal_module.id.as_str()) {
          self.native_cell_modules.insert(module_idx);
        }
        if normal_module.import_records.iter().any(|rec| {
          rec.kind == rolldown_common::ImportKind::Import
            && rec.meta.contains(rolldown_common::ImportRecordMeta::IsExportStar)
        }) {
          self.export_star_modules.insert(module_idx);
        }
        self.late_records |= normal_module.import_records.iter().any(|rec| {
          matches!(
            rec.kind,
            rolldown_common::ImportKind::HotAccept | rolldown_common::ImportKind::DynamicImport
          )
        }) || build_snapshot.index_ecma_ast.get(module_idx).and_then(Option::as_ref).is_some_and(
          |ast| {
            ast.program().body.iter().any(|statement| {
              matches!(
                statement,
                oxc::ast::ast::Statement::ImportDeclaration(import)
                  if import.phase == Some(oxc::ast::ast::ImportPhase::Defer)
              )
            })
          },
        );
        {
          let mut rest = normal_module.ecma_view.source.as_str();
          while let Some(pos) = rest.find("/r/") {
            rest = &rest[pos + 3..];
            if rest.as_bytes().first().is_some_and(u8::is_ascii_digit) {
              self.revision_url_modules.insert(module_idx);
              break;
            }
          }
        }
        if (!normal_module.ecma_view.rolldown_file_url_references.is_empty()
          && !normal_module.id.as_str().ends_with("?__native_url_asset"))
          || normal_module
            .import_records
            .iter()
            .any(|r| r.kind == rolldown_common::ImportKind::NewUrl)
        {
          self.asset_emitter_modules.insert(module_idx);
        }
      }
      self.module_idx_by_stable_id.insert(module.stable_id().clone(), module.idx());
    }
  }

  pub fn create_fast_native_relink_placeholder_output(&self) -> NormalizedScanStageOutput {
    let cache = self.snapshot.as_ref().unwrap();
    NormalizedScanStageOutput {
      module_table: rolldown_common::ModuleTable::default(),
      index_ecma_ast: IndexVec::default(),
      stmt_infos: IndexVec::default(),
      entry_points: Vec::new(),
      symbol_ref_db: rolldown_common::SymbolRefDb::default(),
      runtime: cache.runtime.clone(),
      warnings: Vec::new(),
      dynamic_import_exports_usage_map: FxHashMap::default(),
      overrode_preserve_entry_signature_map: FxHashMap::default(),
      entry_point_to_reference_ids: FxHashMap::default(),
      flat_options: cache.flat_options,
      user_defined_entry_modules: FxHashSet::default(),
      tla_module_count: cache.tla_module_count,
      tla_keyword_span_map: FxHashMap::default(),
    }
  }

  /// # Panic
  /// the function will panic if cache is unset
  pub fn create_output(&mut self) -> NormalizedScanStageOutput {
    use rolldown_utils::rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
    self.invalidate_native_relink();
    let cache = self.snapshot.as_mut().unwrap();
    // Only clone the mutated part of symbol_ref_db
    let symbol_ref_db_partial = cache.symbol_ref_db.clone_without_scoping();
    let symbol_ref_db = std::mem::take(&mut cache.symbol_ref_db);
    cache.symbol_ref_db = symbol_ref_db_partial;

    NormalizedScanStageOutput {
      module_table: rolldown_common::ModuleTable {
        modules: IndexVec::from_vec(
          cache
            .module_table
            .modules
            .raw
            .par_iter_mut()
            .map(|module| match module {
              Module::Normal(normal) => {
                let chain = std::mem::take(&mut normal.sourcemap_chain);
                let mut cloned = normal.clone();
                cloned.sourcemap_chain = chain;
                Module::Normal(cloned)
              }
              Module::External(ext) => Module::External(ext.clone()),
            })
            .collect(),
        ),
      },
      index_ecma_ast: IndexVec::default(),
      stmt_infos: IndexVec::from_vec(cache.stmt_infos.raw.par_iter().cloned().collect()),

      // Since `AstScope` is immutable in following phase, move it to avoid clone
      entry_points: cache.entry_points.clone(),
      symbol_ref_db,
      runtime: cache.runtime.clone(),
      // TODO: cache warning
      warnings: vec![],
      dynamic_import_exports_usage_map: cache.dynamic_import_exports_usage_map.clone(),
      overrode_preserve_entry_signature_map: cache.overrode_preserve_entry_signature_map.clone(),
      entry_point_to_reference_ids: cache.entry_point_to_reference_ids.clone(),
      flat_options: cache.flat_options,
      user_defined_entry_modules: cache.user_defined_entry_modules.clone(),
      tla_module_count: cache.tla_module_count,
      tla_keyword_span_map: cache.tla_keyword_span_map.clone(),
    }
  }
}
