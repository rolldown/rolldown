//! `output.codeSplitting.experimentalInlineCommonChunks`.
//!
//! A selected common chunk ("record") is not written as a file. Its module bodies are printed,
//! once per reading file, inside a registry factory (`__share(id, (exports) => { ... })`), and
//! every reader obtains the shared exports object through `__share_require(id)` ("bridge"). The
//! decisions live here, separate from `ChunkGraph`: `module_to_chunk` and `chunk.modules` never
//! change, a record stays a live chunk, and the placement table below answers "is this a file"
//! and "who carries what". See internal-docs/inline-common-chunks/design.md.

mod deconflict;
mod exclude;
mod place;
mod rewire;
mod select;

use arcstr::ArcStr;
use oxc_str::CompactStr;
use rolldown_common::{ChunkIdx, ModuleIdx};
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

pub use place::InlinePlacement;

/// `RD_LOG=rolldown::inline_common_chunks=debug` prints every accept/reject decision.
pub const LOG_TARGET: &str = "rolldown::inline_common_chunks";

#[derive(Debug, Default)]
pub struct InlineCommonChunksState {
  /// Modules matched by `exclude`, plus `.d.ts`/`.d.mts`/`.d.cts` files.
  excluded_modules: FxHashSet<ModuleIdx>,
  /// Selected chunks in execution order.
  records: FxIndexMap<ChunkIdx, InlineRecord>,
  /// File or record -> the records it reads directly, in record execution order.
  readers: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
  /// File -> the records whose factories it prints, dependencies first.
  carried: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
  /// Reader -> record -> the local binding holding `__share_require(id)`.
  bridge_names: FxHashMap<ChunkIdx, FxHashMap<ChunkIdx, CompactStr>>,
  runtime_chunk: Option<ChunkIdx>,
}

#[derive(Debug, Clone)]
pub struct InlineRecord {
  /// Registry id: the chunk `[name]`, made unique within the build.
  pub id: ArcStr,
  /// The factory parameter receiving the registry's exports object.
  pub exports_param: CompactStr,
}

impl Default for InlineRecord {
  fn default() -> Self {
    Self { id: ArcStr::default(), exports_param: CompactStr::new_const("exports") }
  }
}

impl InlineCommonChunksState {
  pub(crate) fn with_excluded_modules(excluded_modules: FxHashSet<ModuleIdx>) -> Self {
    Self { excluded_modules, ..Self::default() }
  }

  pub fn has_records(&self) -> bool {
    !self.records.is_empty()
  }

  pub fn is_record(&self, chunk_idx: ChunkIdx) -> bool {
    self.records.contains_key(&chunk_idx)
  }

  pub fn record(&self, chunk_idx: ChunkIdx) -> Option<&InlineRecord> {
    self.records.get(&chunk_idx)
  }

  pub fn records(&self) -> impl ExactSizeIterator<Item = (ChunkIdx, &InlineRecord)> {
    self.records.iter().map(|(idx, record)| (*idx, record))
  }

  pub fn record_indices(&self) -> impl ExactSizeIterator<Item = ChunkIdx> + '_ {
    self.records.keys().copied()
  }

  /// The records `chunk_idx` (a file or a record) reads directly.
  pub fn readers_of(&self, chunk_idx: ChunkIdx) -> &[ChunkIdx] {
    self.readers.get(&chunk_idx).map_or(&[], Vec::as_slice)
  }

  /// The records whose factories the file `chunk_idx` prints, dependencies first.
  pub fn carried_by(&self, chunk_idx: ChunkIdx) -> &[ChunkIdx] {
    self.carried.get(&chunk_idx).map_or(&[], Vec::as_slice)
  }

  /// Every file that reads at least one record. Sorted by chunk index.
  pub fn carriers(&self) -> Vec<ChunkIdx> {
    let mut carriers = self.carried.keys().copied().collect::<Vec<_>>();
    carriers.sort_unstable();
    carriers
  }

  /// The bridge binding `reader` uses for symbols owned by `record`, when `reader` reads it.
  pub fn bridge_name(&self, reader: ChunkIdx, record: ChunkIdx) -> Option<&CompactStr> {
    self.bridge_names.get(&reader)?.get(&record)
  }

  /// `Some(bridge)` when `owner_chunk` is a record that `reader` reads through a bridge. A
  /// symbol whose owner chunk is the reader itself, or is a plain file, is not bridged.
  pub fn bridge_for_symbol_owner(
    &self,
    reader: ChunkIdx,
    owner_chunk: ChunkIdx,
  ) -> Option<&CompactStr> {
    if reader == owner_chunk || !self.is_record(owner_chunk) {
      return None;
    }
    self.bridge_name(reader, owner_chunk)
  }

  pub fn runtime_chunk(&self) -> Option<ChunkIdx> {
    self.runtime_chunk
  }

  pub(crate) fn is_excluded_module(&self, module_idx: ModuleIdx) -> bool {
    self.excluded_modules.contains(&module_idx)
  }

  pub(crate) fn set_selection(
    &mut self,
    records: impl IntoIterator<Item = ChunkIdx>,
    placement: InlinePlacement,
    runtime_chunk: ChunkIdx,
  ) {
    self.records = records.into_iter().map(|idx| (idx, InlineRecord::default())).collect();
    self.set_placement(placement);
    self.runtime_chunk = Some(runtime_chunk);
  }

  pub(crate) fn set_placement(&mut self, placement: InlinePlacement) {
    self.readers = placement.readers;
    self.carried = placement.carried;
  }

  pub(crate) fn set_record_id(&mut self, chunk_idx: ChunkIdx, id: ArcStr) {
    self.records.get_mut(&chunk_idx).expect("record should be selected").id = id;
  }

  pub(crate) fn set_record_exports_param(
    &mut self,
    chunk_idx: ChunkIdx,
    exports_param: CompactStr,
  ) {
    self.records.get_mut(&chunk_idx).expect("record should be selected").exports_param =
      exports_param;
  }

  pub(crate) fn set_bridge_names(
    &mut self,
    reader: ChunkIdx,
    bridge_names: FxHashMap<ChunkIdx, CompactStr>,
  ) {
    if !bridge_names.is_empty() {
      self.bridge_names.insert(reader, bridge_names);
    }
  }
}
