use std::collections::VecDeque;

use arcstr::ArcStr;
use itertools::Itertools;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, Module, ModuleIdx, ModuleTable, PreserveEntrySignatures,
};
use rolldown_utils::{BitSet, indexmap::FxIndexMap};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::{GenerateStage, chunk_ext::ChunkCreationReason, chunk_ext::ChunkDebugExt};

impl GenerateStage<'_> {
  /// Constructs a mapping from static entry chunks to the dynamic entry chunks they can reach.
  ///
  /// This is used to determine if a common module can be safely merged into an entry chunk
  /// when that entry chunk can reach all the dynamic entries that also need the module.
  fn construct_static_entry_to_reached_dynamic_entries_map(
    &self,
    chunk_graph: &ChunkGraph,
  ) -> FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> {
    let mut ret: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> = FxHashMap::default();
    let dynamic_entry_modules = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter_map(|(idx, chunk)| match chunk.kind {
        ChunkKind::EntryPoint { meta, module, .. } => {
          (!meta.contains(ChunkMeta::UserDefinedEntry)).then_some((module, idx))
        }
        ChunkKind::Common => None,
      })
      .collect::<FxHashMap<ModuleIdx, ChunkIdx>>();
    for entry in self.link_output.entries.iter().filter(|item| item.kind.is_user_defined()) {
      let Some(entry_chunk_idx) = chunk_graph.module_to_chunk[entry.idx] else {
        continue;
      };
      let mut q = VecDeque::from_iter([entry.idx]);
      let mut visited = FxHashSet::default();
      while let Some(cur) = q.pop_front() {
        if visited.contains(&cur) {
          continue;
        }
        visited.insert(cur);
        let Module::Normal(module) = &self.link_output.module_table[cur] else {
          continue;
        };

        for rec in &module.import_records {
          // Can't put it at the beginning of the loop,
          if let Some(chunk_idx) = dynamic_entry_modules.get(&rec.resolved_module) {
            ret.entry(entry_chunk_idx).or_default().insert(*chunk_idx);
          }
          q.push_back(rec.resolved_module);
        }
      }
    }
    ret
  }

  /// Tries to insert common modules into existing entry chunks instead of creating new common chunks.
  ///
  /// This optimization reduces the total number of chunks by merging shared modules into an appropriate
  /// entry chunk when possible. The function iterates through pending common chunks and either:
  /// 1. Merges modules into an existing entry chunk if safe to do so
  /// 2. Creates a new common chunk if no suitable merge target is found
  pub(super) fn try_insert_common_module_to_exist_chunk(
    &self,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
    pending_common_chunks: FxIndexMap<BitSet, Vec<ModuleIdx>>,
  ) {
    let static_entry_chunk_reference: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> =
      self.construct_static_entry_to_reached_dynamic_entries_map(chunk_graph);

    let entry_chunk_idx =
      chunk_graph.chunk_table.iter_enumerated().map(|(idx, _)| idx).collect::<FxHashSet<_>>();
    // extract entry chunk module relation
    // this means `key_chunk` also referenced all entry module in value `vec`
    for (bits, modules) in pending_common_chunks {
      let chunk_idxs = bits
        .index_of_one()
        .into_iter()
        .map(ChunkIdx::from_raw)
        // Some of the bits maybe not created yet, so filter it out.
        // refer https://github.com/rolldown/rolldown/blob/d373794f5ce5b793ac751bbfaf101cc9cdd261d9/crates/rolldown/src/stages/generate_stage/code_splitting.rs?plain=1#L311-L313
        .filter(|idx| entry_chunk_idx.contains(idx))
        .collect_vec();

      let merge_target = Self::try_insert_into_existing_chunk(
        &chunk_idxs,
        &static_entry_chunk_reference,
        chunk_graph,
        &self.link_output.module_table,
      );

      self.assign_modules_to_chunk(
        merge_target,
        &chunk_idxs,
        modules,
        bits,
        chunk_graph,
        bits_to_chunk,
        input_base,
      );
    }
  }

  /// Assigns modules to either an existing entry chunk or a new common chunk.
  ///
  /// If a valid merge target is found (and it doesn't have strict entry signature preservation),
  /// modules are merged into that existing chunk. Otherwise, a new common chunk is created.
  #[expect(clippy::too_many_arguments)]
  fn assign_modules_to_chunk(
    &self,
    merge_target: Option<ChunkIdx>,
    chunk_idxs: &[ChunkIdx],
    modules: Vec<ModuleIdx>,
    bits: BitSet,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
  ) {
    match merge_target {
      Some(chunk_idx) => {
        let chunk = &chunk_graph.chunk_table[chunk_idx];
        let is_async_entry_only = matches!(chunk.kind, ChunkKind::EntryPoint { meta, .. } if meta == ChunkMeta::DynamicImported);
        if matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::Strict)) {
          // 1. If the target chunk is an async entry, we can merge safely.
          // 2. If the user defined entry chunk has strict preserve entry signature, but all pending
          // modules will not change the entry signature after merge into it, we can still merge them.
          if is_async_entry_only || self.can_merge_without_changing_entry_signature(chunk, &modules)
          {
            self.merge_modules_into_existing_chunk(chunk_idx, chunk_idxs, modules, chunk_graph);
          } else {
            self.create_common_chunk(modules, bits, chunk_graph, bits_to_chunk, input_base);
          }
        } else {
          self.merge_modules_into_existing_chunk(chunk_idx, chunk_idxs, modules, chunk_graph);
        }
      }
      _ => {
        self.create_common_chunk(modules, bits, chunk_graph, bits_to_chunk, input_base);
      }
    }
  }

  /// Merges modules into an existing entry chunk.
  ///
  /// Also initializes imports_from_other_chunks entries for user-defined entry chunks
  /// that will reference the merged chunk, ensuring proper chunk ordering and execution.
  fn merge_modules_into_existing_chunk(
    &self,
    target_chunk_idx: ChunkIdx,
    chunk_idxs: &[ChunkIdx],
    modules: Vec<ModuleIdx>,
    chunk_graph: &mut ChunkGraph,
  ) {
    for idx in chunk_idxs.iter().copied().filter(|idx| *idx != target_chunk_idx) {
      let Some(chunk) = chunk_graph.chunk_table.get_mut(idx) else {
        continue;
      };
      if !matches!(chunk.kind, ChunkKind::EntryPoint { meta, ..} if meta.contains(ChunkMeta::UserDefinedEntry))
      {
        continue;
      }
      chunk.imports_from_other_chunks.entry(target_chunk_idx).or_default();
    }

    for module_idx in modules {
      chunk_graph.add_module_to_chunk(
        module_idx,
        target_chunk_idx,
        self.link_output.metas[module_idx].depended_runtime_helper,
      );
    }
  }

  /// Creates a new common chunk and assigns modules to it.
  fn create_common_chunk(
    &self,
    modules: Vec<ModuleIdx>,
    bits: BitSet,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
  ) {
    let mut chunk =
      Chunk::new(None, None, bits.clone(), vec![], ChunkKind::Common, input_base.clone(), None);
    chunk.add_creation_reason(
      ChunkCreationReason::CommonChunk { bits: &bits, link_output: self.link_output },
      self.options,
    );
    let chunk_id = chunk_graph.add_chunk(chunk);
    for module_idx in modules {
      chunk_graph.add_module_to_chunk(
        module_idx,
        chunk_id,
        self.link_output.metas[module_idx].depended_runtime_helper,
      );
    }
    bits_to_chunk.insert(bits, chunk_id);
  }

  /// Attempts to find an existing entry chunk that can absorb modules shared between multiple entries.
  ///
  /// This optimization reduces the number of chunks by merging shared modules into an appropriate
  /// entry chunk when possible, rather than creating a separate common chunk.
  ///
  /// Returns `Some(ChunkIdx)` if a suitable merge target is found, `None` otherwise.
  fn try_insert_into_existing_chunk(
    chunk_idxs: &[ChunkIdx],
    entry_chunk_reference: &FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>>,
    chunk_graph: &ChunkGraph,
    module_table: &ModuleTable,
  ) -> Option<ChunkIdx> {
    let mut user_defined_entry = vec![];
    let mut dynamic_entry = vec![];
    for &idx in chunk_idxs {
      let Some(chunk) = chunk_graph.chunk_table.get(idx) else {
        continue;
      };
      match chunk.kind {
        ChunkKind::EntryPoint { meta, .. } => {
          if meta.contains(ChunkMeta::UserDefinedEntry) {
            user_defined_entry.push(idx);
          } else {
            dynamic_entry.push(idx);
          }
        }
        ChunkKind::Common => return None,
      }
    }
    let user_defined_entry_modules = Self::collect_entry_modules(&user_defined_entry, chunk_graph)?;

    let merged_user_defined_chunk =
      Self::find_merge_target(&user_defined_entry, &user_defined_entry_modules, module_table);
    if !user_defined_entry.is_empty() {
      let chunk_idx = merged_user_defined_chunk?;

      let ret = dynamic_entry.iter().all(|idx| {
        entry_chunk_reference
          .get(&chunk_idx)
          .map(|reached_dynamic_chunk| reached_dynamic_chunk.contains(idx))
          .unwrap_or(false)
      });
      return ret.then_some(chunk_idx);
    }

    let dynamic_chunk_entry_modules = Self::collect_entry_modules(&dynamic_entry, chunk_graph)?;
    Self::find_merge_target(&dynamic_entry, &dynamic_chunk_entry_modules, module_table)
  }

  /// Collects entry module indices from a list of chunk indices.
  /// Returns `None` if any chunk is missing or has no entry module.
  fn collect_entry_modules(
    chunk_indices: &[ChunkIdx],
    chunk_graph: &ChunkGraph,
  ) -> Option<Vec<ModuleIdx>> {
    let mut ret = Vec::with_capacity(chunk_indices.len());
    for chunk_idx in chunk_indices {
      let chunk = chunk_graph.chunk_table.get(*chunk_idx)?;
      ret.push(chunk.entry_module_idx()?);
    }
    Some(ret)
  }

  /// Checks if merging the given modules into an entry chunk would change the entry's export signature.
  ///
  /// With `preserveEntrySignatures: 'strict'`, we need to ensure that merging modules doesn't add
  /// new exports to the entry chunk. A module is safe to merge if:
  /// 1. It has no exports of its own (purely internal implementation code), OR
  /// 2. All its exports are already part of the entry's resolved exports (re-exported by the entry)
  fn can_merge_without_changing_entry_signature(
    &self,
    chunk: &Chunk,
    modules: &[ModuleIdx],
  ) -> bool {
    let Some(entry_module_idx) = chunk.entry_module_idx() else {
      return false;
    };
    let metas = &self.link_output.metas;
    let module_table = &self.link_output.module_table;

    let entry_exports = &metas[entry_module_idx].resolved_exports;

    modules.iter().all(|&module_idx| {
      // Skip the entry module itself - it's always safe
      if module_idx == entry_module_idx || module_table[module_idx].as_normal().is_none() {
        return true;
      }

      let module_meta = &metas[module_idx];

      // A module is safe to merge if all its exports are already covered by the entry's exports.
      // This means either:
      // 1. The module has no exports (empty resolved_exports)
      // 2. All of the module's exports point to symbols that the entry also exports
      module_meta.resolved_exports.iter().all(|(export_name, resolved_export)| {
        // Check if the entry has an export with the same name that resolves to the same symbol
        entry_exports
          .get(export_name)
          .is_some_and(|entry_export| entry_export.symbol_ref == resolved_export.symbol_ref)
      })
    })
  }

  /// Finds a chunk that can serve as the merge target for all entries.
  /// A chunk can be the merge target if its entry module is imported by or equal to all other entry modules.
  fn find_merge_target(
    chunk_indices: &[ChunkIdx],
    entry_modules: &[ModuleIdx],
    module_table: &ModuleTable,
  ) -> Option<ChunkIdx> {
    chunk_indices.iter().zip(entry_modules.iter()).find_map(|(chunk_idx, entry_module_idx)| {
      let module = module_table[*entry_module_idx].as_normal().expect("Should be normal module");
      let can_merge = entry_modules.iter().all(|other_entry_module_idx| {
        *entry_module_idx == *other_entry_module_idx
          || module.importers_idx.contains(other_entry_module_idx)
      });
      can_merge.then_some(*chunk_idx)
    })
  }
}
