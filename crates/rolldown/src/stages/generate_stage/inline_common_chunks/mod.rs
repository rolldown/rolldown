//! `output.codeSplitting.experimentalInlineCommonChunks`.
//!
//! A selected common chunk ("record") is not written as a file. Every file that carries it (a
//! reader, unless every known entry loading path already registers it) prints its
//! own copy of the record's modules inside a registry factory (`__share(id, (exports) => {
//! ... })`), finalized and named as part of that file, and every reader obtains the shared
//! exports object through `__share_require(id)` ("bridge"). The decisions live here, separate from `ChunkGraph`:
//! `module_to_chunk` and `chunk.modules` never change, a record stays a live chunk, and the
//! placement table below answers "is this a file" and "who carries what". See
//! internal-docs/inline-common-chunks/design.md.

mod callee_guard;
mod exclude;
mod finalize;
mod naming;
mod place;
mod rewire;
mod select;

use arcstr::ArcStr;
use itertools::Itertools;
use oxc_str::CompactStr;
use rolldown_common::{ChunkIdx, ModuleIdx, SymbolRef, SymbolRefDb};
use rolldown_utils::{base64::to_url_safe_base64, concat_string, indexmap::FxIndexMap};
use rustc_hash::{FxHashMap, FxHashSet};
use xxhash_rust::xxh3::Xxh3;

use crate::chunk_graph::ChunkGraph;

pub use callee_guard::{BridgeCallees, bare_bridge_callee};
pub use finalize::CarriedRender;
pub use place::InlinePlacement;

/// `RD_LOG=rolldown::inline_common_chunks=debug` prints every accept/reject decision.
pub const LOG_TARGET: &str = "rolldown::inline_common_chunks";

#[derive(Debug, Default)]
pub struct InlineCommonChunksState {
  /// Modules matched by `exclude`, plus `.d.ts`/`.d.mts`/`.d.cts` files.
  excluded_modules: FxHashSet<ModuleIdx>,
  /// Selected chunks in execution order.
  records: FxIndexMap<ChunkIdx, InlineRecord>,
  /// Who reads and who carries each record, computed at selection.
  placement: InlinePlacement,
  /// File -> the names its deconflict pass chose for the feature's bindings.
  names: FxHashMap<ChunkIdx, FileInlineNames>,
  runtime_chunk: Option<ChunkIdx>,
}

#[derive(Debug, Default)]
pub struct InlineRecord {
  /// Registry id, see [`record_id`].
  pub id: ArcStr,
}

/// The bindings `experimentalInlineCommonChunks` adds to one output file,
/// named by that file's deconflict pass together with everything else it declares.
#[derive(Debug)]
pub struct FileInlineNames {
  /// The parameter every factory printed in the file receives the registry's exports object in.
  pub exports_param: CompactStr,
  /// Record -> the file-level binding holding `__share_require(id)`.
  pub bridges: FxHashMap<ChunkIdx, CompactStr>,
  /// Carried record -> (record its factory reads -> the factory-local binding).
  pub factory_bridges: FxHashMap<ChunkIdx, FxHashMap<ChunkIdx, CompactStr>>,
}

/// Who a piece of code is when it reads a record: `file` is the output file the code is printed
/// in (whose names it uses), `reader` is the chunk whose imports it resolves through bridges. The
/// two differ for a record's modules printed inside a factory: their names come from the carrier,
/// their bridges are the factory's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlineReader {
  pub file: ChunkIdx,
  pub reader: ChunkIdx,
}

/// The hash characters in a record id, as many as `[hash]` puts in a file name.
const RECORD_ID_HASH_LEN: usize = 8;

/// A record's registry id: its chunk `[name]` plus [`RECORD_ID_HASH_LEN`] characters of the hash
/// of its module set (the members' stable ids, sorted), `shared-CNoTWEik`. The id identifies the
/// record within its build and stays the same across builds while the module set does, so a
/// carrier's bytes move only with its own content; it says nothing about the record's code. Like
/// webpack's module ids, it is not meant to tell two builds apart: files of two builds that meet
/// in one page and import the same runtime file share one registry, and that is not supported.
/// Two records of one build with the same name and hash characters (a hash collision, since
/// their module sets differ) rehash until the ids differ. See
/// internal-docs/inline-common-chunks/implementation.md.
pub(super) fn record_id<'a>(
  chunk_name: &str,
  module_stable_ids: impl Iterator<Item = &'a str>,
  taken: &mut FxHashSet<ArcStr>,
) -> ArcStr {
  let mut hasher = Xxh3::default();
  for stable_id in module_stable_ids.sorted_unstable() {
    hasher.update(stable_id.as_bytes());
    hasher.update(b"\0");
  }
  loop {
    let hash = to_url_safe_base64(hasher.digest128().to_le_bytes());
    let id = ArcStr::from(concat_string!(chunk_name, "-", &hash[..RECORD_ID_HASH_LEN]));
    if taken.insert(id.clone()) {
      return id;
    }
    hasher.update(hash.as_bytes());
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

  pub fn record_indices(&self) -> impl ExactSizeIterator<Item = ChunkIdx> + '_ {
    self.records.keys().copied()
  }

  /// The records `chunk_idx` (a file or a record) reads directly.
  pub fn readers_of(&self, chunk_idx: ChunkIdx) -> &[ChunkIdx] {
    self.placement.readers.get(&chunk_idx).map_or(&[], Vec::as_slice)
  }

  /// The records whose factories the file `chunk_idx` prints, dependencies first.
  pub fn carried_by(&self, chunk_idx: ChunkIdx) -> &[ChunkIdx] {
    self.placement.carried.get(&chunk_idx).map_or(&[], Vec::as_slice)
  }

  /// Every file that reads at least one record, whether or not it prints a factory. Sorted by
  /// chunk index.
  pub fn reading_files(&self) -> &[ChunkIdx] {
    &self.placement.reading_files
  }

  pub(super) fn placement(&self) -> &InlinePlacement {
    &self.placement
  }

  /// Every file that prints at least one factory. Sorted by chunk index.
  pub fn carriers(&self) -> Vec<ChunkIdx> {
    let mut carriers = self.placement.carried.keys().copied().collect::<Vec<_>>();
    carriers.sort_unstable();
    carriers
  }

  pub fn file_names(&self, file: ChunkIdx) -> Option<&FileInlineNames> {
    self.names.get(&file)
  }

  /// Every bridge binding declared in `file`, the file's own and its factories', each with the
  /// export names its record binds to a symbol in `wrapper_refs` (canonical). Empty for a chunk
  /// that reads no record.
  pub fn bridge_callees(
    &self,
    file: ChunkIdx,
    chunk_graph: &ChunkGraph,
    wrapper_refs: &FxHashSet<SymbolRef>,
    symbol_db: &SymbolRefDb,
  ) -> BridgeCallees {
    let mut callees = BridgeCallees::default();
    let Some(names) = self.names.get(&file) else {
      return callees;
    };
    let file_bridges = names.bridges.iter();
    let factory_bridges = names.factory_bridges.values().flatten();
    for (record, bridge) in file_bridges.chain(factory_bridges) {
      let wrappers = callees.add_bridge(bridge);
      for (symbol_ref, export_names) in &chunk_graph.chunk_table[*record].exports_to_other_chunks {
        if wrapper_refs.contains(&symbol_db.canonical_ref_for(*symbol_ref)) {
          wrappers.extend(export_names.iter().cloned());
        }
      }
    }
    callees
  }

  /// `(bridge, export name)` when `canonical_ref` is owned by a record that `reader` reads through
  /// a bridge: the reference renders as `bridge.export_name`, a live getter read. A symbol owned
  /// by the reader itself or by a plain file is not bridged.
  pub fn bridge_read<'a>(
    &'a self,
    reader: InlineReader,
    canonical_ref: SymbolRef,
    symbol_db: &SymbolRefDb,
    chunk_graph: &'a ChunkGraph,
  ) -> Option<(&'a CompactStr, &'a CompactStr)> {
    let owner_chunk = symbol_db.get(canonical_ref).chunk_idx?;
    let bridge = self.bridge_for_symbol_owner(reader, owner_chunk)?;
    let export_name =
      chunk_graph.chunk_table[owner_chunk].exports_to_other_chunks.get(&canonical_ref)?.first()?;
    Some((bridge, export_name))
  }

  fn bridge_for_symbol_owner(
    &self,
    reader: InlineReader,
    owner_chunk: ChunkIdx,
  ) -> Option<&CompactStr> {
    if reader.reader == owner_chunk || !self.is_record(owner_chunk) {
      return None;
    }
    let names = self.names.get(&reader.file)?;
    let bridges = if reader.file == reader.reader {
      &names.bridges
    } else {
      names.factory_bridges.get(&reader.reader)?
    };
    bridges.get(&owner_chunk)
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
    self.placement = placement;
    self.runtime_chunk = Some(runtime_chunk);
  }

  pub(crate) fn set_record_id(&mut self, chunk_idx: ChunkIdx, id: ArcStr) {
    self.records.get_mut(&chunk_idx).expect("record should be selected").id = id;
  }

  pub(crate) fn set_file_names(&mut self, file: ChunkIdx, names: FileInlineNames) {
    self.names.insert(file, names);
  }
}
