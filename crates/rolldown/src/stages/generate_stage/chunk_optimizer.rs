use std::collections::{VecDeque, hash_map::Entry};

use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Chunk, ChunkDebugInfo, ChunkIdx, ChunkKind, ChunkMeta, ChunkReasonType,
  FacadeChunkEliminationReason, Module, ModuleIdx, ModuleNamespaceIncludedReason, ModuleTable,
  PostChunkOptimizationOperation, PreserveEntrySignatures, RuntimeHelper, StmtInfos, WrapKind,
};
use rolldown_utils::{BitSet, indexmap::FxIndexMap};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  stages::link_stage::{
    IncludeContext, SymbolIncludeReason, include_runtime_symbol, include_symbol,
  },
  types::linking_metadata::{
    LinkingMetadata, included_info_to_linking_metadata_vec, linking_metadata_vec_to_included_info,
  },
};

use super::{
  GenerateStage, chunk_ext::ChunkCreationReason, chunk_ext::ChunkDebugExt,
  code_splitting::IndexSplittingInfo,
};

#[derive(Debug, Default)]
/// A lightweight representation of a chunk used during optimization passes.
struct ChunkCandidate {
  modules: Vec<ModuleIdx>,
  /// Whether this chunk needs to be created in the final chunk graph.
  needs_creation: bool,
  dependencies: FxHashSet<ChunkIdx>,
}

type TempIndexChunks = IndexVec<ChunkIdx, ChunkCandidate>;

/// A temporary structure for managing chunk graph optimizations.
/// Only store simplified information needed during optimization passes.
#[derive(Debug, Default)]
pub struct ChunkOptimizationGraph {
  chunks: TempIndexChunks,
  bits_to_chunk_idx: FxIndexMap<BitSet, ChunkIdx>,
  /// Mapping from module index to the chunk it belongs to.
  module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
  /// Mapping from chunk_graph indices to temp_chunk_graph indices.
  /// Initial chunks share identical indices; newly-created common chunks
  /// in chunk_graph are registered here so callers can translate.
  chunk_idx_to_temp_chunk_idx: FxHashMap<ChunkIdx, ChunkIdx>,
}

impl ChunkOptimizationGraph {
  pub fn new(
    chunk_optimization: bool,
    chunk_graph: &ChunkGraph,
    bits_to_chunk_idx: &FxHashMap<BitSet, ChunkIdx>,
  ) -> Self {
    if !chunk_optimization {
      return Self::default();
    }
    // These initial chunks already exist in the chunk graph, including:
    // - entry chunks
    // - manual code splitting chunks
    let mut module_to_chunk = index_vec![None; chunk_graph.module_to_chunk.len()];
    let mut chunk_idx_to_temp_chunk_idx = FxHashMap::default();
    let chunks = chunk_graph
      .chunk_table
      .iter_enumerated()
      .map(|(chunk_idx, item)| {
        for &module_idx in &item.modules {
          module_to_chunk[module_idx] = Some(chunk_idx);
        }
        // Initial chunks have identical indices in both graphs
        chunk_idx_to_temp_chunk_idx.insert(chunk_idx, chunk_idx);
        ChunkCandidate {
          modules: item.modules.clone(),
          needs_creation: false,
          dependencies: FxHashSet::default(),
        }
      })
      .collect();
    Self {
      chunks,
      bits_to_chunk_idx: bits_to_chunk_idx.iter().map(|(k, v)| (k.clone(), *v)).collect(),
      module_to_chunk,
      chunk_idx_to_temp_chunk_idx,
    }
  }

  /// Assigns a module to a temporary chunk based on its reachability bits.
  ///
  /// If a chunk already exists for the given bit pattern, the module is added to it.
  /// Otherwise, a new temporary chunk is created and marked as `needs_created: true`,
  /// indicating it will need to be materialized in the final chunk graph.
  pub fn init_module_assignment(&mut self, module_idx: ModuleIdx, bits: &BitSet) {
    if let Some(&chunk_idx) = self.bits_to_chunk_idx.get(bits) {
      self.chunks[chunk_idx].modules.push(module_idx);
      self.module_to_chunk[module_idx] = Some(chunk_idx);
    } else {
      let temp_chunk = ChunkCandidate {
        modules: vec![module_idx],
        needs_creation: true,
        dependencies: FxHashSet::default(),
      };
      let chunk_idx = self.chunks.push(temp_chunk);
      self.bits_to_chunk_idx.insert(bits.clone(), chunk_idx);
      self.module_to_chunk[module_idx] = Some(chunk_idx);
    }
  }

  pub fn add_module_to_chunk(&mut self, module_idx: ModuleIdx, chunk_idx: ChunkIdx) {
    self.chunks[chunk_idx].modules.push(module_idx);
    self.module_to_chunk[module_idx] = Some(chunk_idx);
  }

  /// Records the mapping from a newly-created chunk_graph index to its
  /// corresponding temp_chunk_graph index.
  pub fn register_chunk_graph_index(
    &mut self,
    chunk_graph_idx: ChunkIdx,
    temp_chunk_idx: ChunkIdx,
  ) {
    self.chunk_idx_to_temp_chunk_idx.insert(chunk_graph_idx, temp_chunk_idx);
  }

  /// Translates a chunk_graph index into the corresponding temp_chunk_graph index.
  /// Returns `None` if the chunk_graph index has no known temp counterpart.
  pub fn to_temp_idx(&self, chunk_graph_idx: ChunkIdx) -> Option<ChunkIdx> {
    self.chunk_idx_to_temp_chunk_idx.get(&chunk_graph_idx).copied()
  }

  /// Calculates chunk dependencies based on module dependencies.
  ///
  /// For each chunk, iterates through its modules and their dependencies.
  /// If a module dependency belongs to a different chunk, that chunk is added
  /// as a dependency of the current chunk.
  ///
  /// This is similar to Rollup's `addChunkDependenciesAndGetExternalSideEffectAtoms`.
  pub fn calc_chunk_dependencies(&mut self, metas: &IndexVec<ModuleIdx, LinkingMetadata>) {
    for chunk_idx in self.chunks.indices() {
      let modules = std::mem::take(&mut self.chunks[chunk_idx].modules);
      for module_idx in &modules {
        let module_dependencies = &metas[*module_idx].dependencies;
        for &dep_module_idx in module_dependencies {
          if let Some(dep_chunk_idx) = self.module_to_chunk[dep_module_idx] {
            // Only add if dependency is in a different chunk
            if dep_chunk_idx != chunk_idx {
              self.chunks[chunk_idx].dependencies.insert(dep_chunk_idx);
            }
          }
        }
      }
      self.chunks[chunk_idx].modules = modules;
    }
  }

  /// Checks if merging `source_chunk` into `target_chunk` would create a circular dependency.
  ///
  /// Returns `true` if the merge would create a cycle, `false` if it's safe to merge.
  ///
  /// A circular dependency would occur if `target_chunk` is a transitive dependency of `source_chunk`.
  /// This is similar to Rollup's check in `getAdditionalSizeIfNoTransitiveDependencyOrNonCorrelatedSideEffect`.
  pub fn would_create_circular_dependency(
    &self,
    source_chunk_idx: ChunkIdx,
    target_chunk_idx: ChunkIdx,
  ) -> bool {
    // BFS to check if target_chunk is reachable from source_chunk's dependencies
    let mut chunks_to_check: VecDeque<ChunkIdx> =
      self.chunks[source_chunk_idx].dependencies.iter().copied().collect();
    let mut visited = FxHashSet::default();

    while let Some(dep_chunk_idx) = chunks_to_check.pop_front() {
      if dep_chunk_idx == target_chunk_idx {
        // Found target_chunk in the transitive dependencies of source_chunk
        // Merging would create a circular dependency
        return true;
      }

      if visited.contains(&dep_chunk_idx) {
        continue;
      }
      visited.insert(dep_chunk_idx);

      // Add this chunk's dependencies to the queue
      for &next_dep in &self.chunks[dep_chunk_idx].dependencies {
        if !visited.contains(&next_dep) {
          chunks_to_check.push_back(next_dep);
        }
      }
    }

    false
  }

  /// Merges the dependencies of source chunk into target chunk.
  ///
  /// When modules from one chunk are merged into another chunk, the dependencies
  /// of the source chunk should also be merged into the target chunk.
  /// Additionally, removes the target chunk from its own dependencies (self-reference).
  pub fn merge_chunk_dependencies(
    &mut self,
    target_chunk_idx: ChunkIdx,
    source_chunk_idx: ChunkIdx,
  ) {
    let source_dependencies = std::mem::take(&mut self.chunks[source_chunk_idx].dependencies);
    for dep_chunk_idx in source_dependencies {
      // Don't add self-reference
      if dep_chunk_idx != target_chunk_idx {
        self.chunks[target_chunk_idx].dependencies.insert(dep_chunk_idx);
      }
    }
    // Remove self-reference if it exists (source might have depended on target)
    self.chunks[target_chunk_idx].dependencies.remove(&target_chunk_idx);
  }
}

/// Result of assigning modules during chunk optimization.
enum ChunkAssignment {
  /// Modules were merged into an existing entry chunk (chunk_graph index).
  Merged(ChunkIdx),
  /// A new common chunk was created in chunk_graph (chunk_graph index).
  Created(ChunkIdx),
}

/// Information about a facade chunk merge operation.
/// Contains (source_chunk_idx, target_chunk_idx, elimination_reason).
type FacadeChunkMergeInfo = (ChunkIdx, ChunkIdx, FacadeChunkEliminationReason);

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
    for (&module_idx, _entry_points) in self
      .link_output
      .entries
      .iter()
      .filter(|(_, entries)| entries.iter().any(|e| e.kind.is_user_defined()))
    {
      let Some(entry_chunk_idx) = chunk_graph.module_to_chunk[module_idx] else {
        continue;
      };
      let mut q = VecDeque::from_iter([module_idx]);
      let mut visited = FxHashSet::default();
      while let Some(cur) = q.pop_front() {
        if visited.contains(&cur) {
          continue;
        }
        visited.insert(cur);
        let Module::Normal(module) = &self.link_output.module_table[cur] else {
          continue;
        };

        for dep_idx in module.import_records.iter().filter_map(|r| r.resolved_module) {
          // Can't put it at the beginning of the loop,
          if let Some(chunk_idx) = dynamic_entry_modules.get(&dep_idx) {
            ret.entry(entry_chunk_idx).or_default().insert(*chunk_idx);
          }
          q.push_back(dep_idx);
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
    temp_chunk_graph: &mut ChunkOptimizationGraph,
  ) {
    let static_entry_chunk_reference: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> =
      self.construct_static_entry_to_reached_dynamic_entries_map(chunk_graph);

    let entry_chunk_idx =
      chunk_graph.chunk_table.iter_enumerated().map(|(idx, _)| idx).collect::<FxHashSet<_>>();
    // Calculate on demand to avoid add a new field on each NormalModule.
    let dynamic_entry_to_dynamic_importers: FxHashMap<ModuleIdx, FxHashSet<ModuleIdx>> = {
      // Get dynamic entry modules from chunk_table, then find matched entry points
      // and extract importers from related_stmt_infos
      let mut map: FxHashMap<ModuleIdx, FxHashSet<ModuleIdx>> = FxHashMap::default();
      for chunk in chunk_graph.chunk_table.iter() {
        let ChunkKind::EntryPoint { meta, module, .. } = chunk.kind else {
          continue;
        };
        if meta != ChunkMeta::DynamicImported {
          continue;
        }
        // Find the matched dynamic entry point from link_output.entries
        if let Some(entries) = self.link_output.entries.get(&module) {
          for entry in entries.iter().filter(|e| e.kind.is_dynamic_import()) {
            // Extract importers from related_stmt_infos (first element is the importer ModuleIdx)
            for (importer_idx, _, _, _) in &entry.related_stmt_infos {
              map.entry(module).or_default().insert(*importer_idx);
            }
          }
        }
      }
      map
    };
    // First pass: collect chunk assignment decisions
    // (bits, temp_chunk_idx, chunk_idxs, merge_target)
    let assignments: Vec<_> = temp_chunk_graph
      .bits_to_chunk_idx
      .iter()
      .filter_map(|(bits, temp_chunk_idx)| {
        let temp_chunk = &temp_chunk_graph.chunks[*temp_chunk_idx];
        // Skip those chunks that are already created in chunk graph
        if !temp_chunk.needs_creation {
          return None;
        }
        let chunk_idxs: Vec<_> = bits
          .index_of_one()
          .into_iter()
          .map(ChunkIdx::from_raw)
          // Some of the bits maybe not created yet, so filter it out.
          // refer https://github.com/rolldown/rolldown/blob/d373794f5ce5b793ac751bbfaf101cc9cdd261d9/crates/rolldown/src/stages/generate_stage/code_splitting.rs?plain=1#L311-L313
          .filter(|idx| entry_chunk_idx.contains(idx))
          .collect();

        let merge_target = Self::try_insert_into_existing_chunk(
          &chunk_idxs,
          &static_entry_chunk_reference,
          chunk_graph,
          &self.link_output.module_table,
          &dynamic_entry_to_dynamic_importers,
          temp_chunk,
        );

        Some((bits.clone(), *temp_chunk_idx, chunk_idxs, merge_target))
      })
      .collect();

    // Second pass: apply chunk assignments
    for (bits, temp_chunk_idx, chunk_idxs, merge_target) in assignments {
      // Check if merging would create a circular dependency
      let merge_target = match merge_target {
        Some(target_chunk_idx)
          if temp_chunk_graph
            .would_create_circular_dependency(temp_chunk_idx, target_chunk_idx) =>
        {
          // Skip merge if it would create a circular dependency
          None
        }
        other => other,
      };

      let temp_modules = &temp_chunk_graph.chunks[temp_chunk_idx].modules;
      match self.assign_modules_to_chunk(
        merge_target,
        &chunk_idxs,
        temp_modules,
        &bits,
        chunk_graph,
        bits_to_chunk,
        input_base,
      ) {
        ChunkAssignment::Merged(target_chunk_idx) => {
          // Merge chunk dependencies immediately after successful merge
          temp_chunk_graph.merge_chunk_dependencies(target_chunk_idx, temp_chunk_idx);
        }
        ChunkAssignment::Created(new_chunk_id) => {
          temp_chunk_graph.register_chunk_graph_index(new_chunk_id, temp_chunk_idx);
        }
      }
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
    modules: &[ModuleIdx],
    bits: &BitSet,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
  ) -> ChunkAssignment {
    match merge_target {
      Some(chunk_idx) => {
        let chunk = &chunk_graph.chunk_table[chunk_idx];
        let is_async_entry_only = matches!(chunk.kind, ChunkKind::EntryPoint { meta, .. } if meta == ChunkMeta::DynamicImported);
        if matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::Strict)) {
          // We can safely merge into this chunk in two scenarios:
          // 1. The target chunk is an async entry - dynamic chunks are not restricted by `PreserveEntrySignatures`.
          // 2. The target chunk has strict signature preservation, but the modules being merged won't alter
          //    the entry's exported interface (they either have no exports or only re-export existing entry symbols).
          if is_async_entry_only || self.can_merge_without_changing_entry_signature(chunk, modules)
          {
            self.merge_modules_into_existing_chunk(chunk_idx, chunk_idxs, modules, chunk_graph);
            ChunkAssignment::Merged(chunk_idx)
          } else {
            let new_chunk_id =
              self.create_common_chunk(modules, bits, chunk_graph, bits_to_chunk, input_base);
            ChunkAssignment::Created(new_chunk_id)
          }
        } else {
          self.merge_modules_into_existing_chunk(chunk_idx, chunk_idxs, modules, chunk_graph);
          ChunkAssignment::Merged(chunk_idx)
        }
      }
      _ => {
        let new_chunk_id =
          self.create_common_chunk(modules, bits, chunk_graph, bits_to_chunk, input_base);
        ChunkAssignment::Created(new_chunk_id)
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
    modules: &[ModuleIdx],
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

    for &module_idx in modules {
      chunk_graph.add_module_to_chunk(
        module_idx,
        target_chunk_idx,
        self.link_output.metas[module_idx].depended_runtime_helper,
      );
    }
  }

  /// Creates a new common chunk and assigns modules to it.
  /// Returns the new chunk_graph index.
  fn create_common_chunk(
    &self,
    modules: &[ModuleIdx],
    bits: &BitSet,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
  ) -> ChunkIdx {
    let mut chunk =
      Chunk::new(None, None, bits.clone(), vec![], ChunkKind::Common, input_base.clone(), None);
    chunk.add_creation_reason(
      ChunkCreationReason::CommonChunk { bits, link_output: self.link_output },
      self.options,
    );
    let chunk_id = chunk_graph.add_chunk(chunk);
    for &module_idx in modules {
      chunk_graph.add_module_to_chunk(
        module_idx,
        chunk_id,
        self.link_output.metas[module_idx].depended_runtime_helper,
      );
    }
    bits_to_chunk.insert(bits.clone(), chunk_id);
    chunk_id
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
    dynamic_entry_to_dynamic_importers: &FxHashMap<ModuleIdx, FxHashSet<ModuleIdx>>,
    info: &ChunkCandidate,
  ) -> Option<ChunkIdx> {
    let mut user_defined_entry = vec![];
    let mut dynamic_entry = vec![];
    for &idx in chunk_idxs {
      let Some(chunk) = chunk_graph.chunk_table.get(idx) else {
        continue;
      };
      match chunk.kind {
        ChunkKind::EntryPoint { meta, .. } => {
          if meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk) {
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
    if user_defined_entry.is_empty() {
      let dynamic_chunk_entry_modules = Self::collect_entry_modules(&dynamic_entry, chunk_graph)?;
      Self::find_merge_target(&dynamic_entry, &dynamic_chunk_entry_modules, module_table)
    } else {
      let chunk_idx = merged_user_defined_chunk?;
      let chunk = &chunk_graph.chunk_table[chunk_idx];
      let reached_dynamic_chunk = entry_chunk_reference.get(&chunk_idx);
      // Check if all dynamic entry chunks are reachable from the merged user-defined entry chunk.
      // If not, we cannot merge the shared modules into the user-defined entry chunk because
      // the dynamic entries would not be able to access the shared modules.
      let all_dynamic_entries_reachable =
        dynamic_entry.iter().all(|idx| reached_dynamic_chunk.is_some_and(|set| set.contains(idx)));
      if !all_dynamic_entries_reachable {
        return None;
      }
      let modules_set = chunk
        .modules
        .iter()
        .copied()
        .chain(
          info
            .modules
            .iter()
            .filter(|idx| !dynamic_entry_to_dynamic_importers.contains_key(idx))
            .copied(),
        )
        .collect::<FxHashSet<_>>();
      // For each module in the shared modules, if it is a dynamic entry module,
      // all its dynamic importers must be in the current chunk (including modules to be merged).
      // This ensures that when a dynamic import occurs, the merged entry chunk (containing the
      // dynamic entry module) is already loaded, preventing missing module errors at runtime.
      let all_dynamic_entry_importers_valid = info.modules.iter().all(|&module_idx| {
        let Some(importers) = dynamic_entry_to_dynamic_importers.get(&module_idx) else {
          // Not a dynamic entry module, no constraint
          return true;
        };
        // all importer are in current chunk (includes those modules will be merged)
        importers.iter().all(|importer| modules_set.contains(importer))
      });
      all_dynamic_entry_importers_valid.then_some(chunk_idx)
    }
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

  /// Finds empty dynamic entry chunks that should be merged with their target common chunks.
  /// Returns a tuple of (merge_entry_to_chunk, emitted_chunk_groups).
  fn find_facade_chunk_merge_candidates(
    &self,
    chunk_graph: &ChunkGraph,
    temp_chunk_graph: &ChunkOptimizationGraph,
  ) -> (FxHashMap<ModuleIdx, FacadeChunkMergeInfo>, FxHashMap<ChunkIdx, Vec<ChunkIdx>>) {
    let mut merge_entry_to_chunk = FxHashMap::default();
    let mut emitted_chunk_groups: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
    let temp_runtime_chunk_idx = chunk_graph.module_to_chunk[self.link_output.runtime.id()]
      .and_then(|idx| temp_chunk_graph.to_temp_idx(idx));
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      let ChunkKind::EntryPoint { meta, bit: _, module } = chunk.kind else {
        continue;
      };
      if meta.intersects(ChunkMeta::UserDefinedEntry) {
        continue;
      }
      let is_emitted_from_chunk = if meta.contains(ChunkMeta::EmittedChunk) {
        if matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::AllowExtension)) {
          true
        } else {
          // If an emitted chunk has other `preserveEntrySignatures` values, we can't optimize it.
          // The facade chunk needs to be preserved.
          continue;
        }
      } else {
        false
      };
      if !chunk.modules.is_empty() {
        continue;
      }
      // Check if the entry module is included in a common chunk
      let Some(target_chunk_idx) = chunk_graph.module_to_chunk[module] else {
        continue;
      };
      let target_chunk = &chunk_graph.chunk_table[target_chunk_idx];
      let is_manual_to_chunk = matches!(
        target_chunk.chunk_reason_type.as_ref(),
        ChunkReasonType::ManualCodeSplitting { .. }
      );
      let is_pure_user_defined_to_entry_chunk = matches!(target_chunk.kind, ChunkKind::EntryPoint { meta, bit: _, module: _ } if meta.is_pure_user_defined_entry());
      let is_common_chunk = matches!(target_chunk.kind, ChunkKind::Common);
      // Four optimization scenarios:
      // 1. Emitted chunk (AllowExtension) merged into manual code splitting group
      //    → Group by target chunk to detect export name conflicts before merging
      // 2. Dynamic entry chunk merged into manual code splitting group
      //    → Directly merge, the facade chunk can be removed
      // 3. Dynamic entry chunk merged into user-defined entry chunk
      //    → Directly merge, the facade chunk can be removed
      // 4. Dynamic entry chunk merged into common chunk
      //    → Directly merge, the facade chunk can be removed
      if is_manual_to_chunk && is_emitted_from_chunk {
        emitted_chunk_groups.entry(target_chunk_idx).or_default().push(chunk_idx);
      } else if !is_emitted_from_chunk {
        // Check if merging would create a circular dependency.
        // Translate chunk_graph indices to temp_chunk_graph indices, since the two
        // graphs may diverge once new common chunks are materialised.
        if temp_runtime_chunk_idx
          .and_then(|temp_runtime_idx| {
            let temp_target_idx = temp_chunk_graph.to_temp_idx(target_chunk_idx)?;
            Some(
              temp_chunk_graph.would_create_circular_dependency(temp_runtime_idx, temp_target_idx),
            )
          })
          // If runtime is not included before, it will not create circular dependency, because
          // the runtime module will be either included in the target chunk or in a separate chunk loaded before.
          // If either index has no temp counterpart, we conservatively allow the merge.
          .unwrap_or(false)
        {
          continue;
        }
        let reason = if is_manual_to_chunk {
          FacadeChunkEliminationReason::DynamicEntryMergedIntoManualGroup
        } else if is_pure_user_defined_to_entry_chunk {
          FacadeChunkEliminationReason::DynamicEntryMergedIntoUserDefinedEntry
        } else if is_common_chunk {
          FacadeChunkEliminationReason::DynamicEntryMergedIntoCommonChunk
        } else {
          continue;
        };
        merge_entry_to_chunk.insert(module, (chunk_idx, target_chunk_idx, reason));
      }
    }
    (merge_entry_to_chunk, emitted_chunk_groups)
  }

  /// Batch process emitted chunk groups to detect export name conflicts.
  /// Chunks with conflicting export names need to keep their facade chunks.
  fn process_emitted_chunk_groups(
    &self,
    chunk_graph: &ChunkGraph,
    emitted_chunk_groups: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
    merge_entry_to_chunk: &mut FxHashMap<ModuleIdx, FacadeChunkMergeInfo>,
  ) {
    for (target_chunk_idx, chunk_indices) in emitted_chunk_groups {
      let Some(_target_chunk) = chunk_graph.chunk_table.get(target_chunk_idx) else {
        continue;
      };
      let mut chunk_entry_module_idxs: Vec<(ChunkIdx, ModuleIdx)> = chunk_indices
        .iter()
        .filter_map(|item| {
          let chunk = chunk_graph.chunk_table.get(*item)?;
          let entry_module_idx = chunk.entry_module_idx()?;
          Some((*item, entry_module_idx))
        })
        .collect_vec();
      // Sort by reverse execution order to match the naming order in deconflict_chunk_symbol
      chunk_entry_module_idxs.sort_by_cached_key(|(_idx, module_idx)| {
        std::cmp::Reverse(self.link_output.module_table[*module_idx].exec_order())
      });

      let mut allocated_export_symbol = FxHashMap::default();
      // If an emitted chunk exports a name that conflicts with an already allocated export,
      // its facade chunk cannot be removed.
      for (chunk_idx, entry_module_idx) in chunk_entry_module_idxs {
        let module_meta = &self.link_output.metas[entry_module_idx];
        let needs_facade =
          module_meta.resolved_exports.iter().any(|(export_name, resolved_export)| {
            let canonical_ref =
              resolved_export.symbol_ref.canonical_ref(&self.link_output.symbol_db);
            match allocated_export_symbol.entry(export_name) {
              Entry::Occupied(occupied_entry) => canonical_ref != *occupied_entry.get(),
              Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(canonical_ref);
                false
              }
            }
          });
        if !needs_facade {
          merge_entry_to_chunk.insert(
            entry_module_idx,
            (
              chunk_idx,
              target_chunk_idx,
              FacadeChunkEliminationReason::EmittedChunkMergedIntoManualGroup,
            ),
          );
        }
      }
    }
  }

  /// This optimization handles the case where a dynamic entry chunk has no modules of its own
  /// because all of its modules were moved to a common chunk (when dynamic entry modules are captured by `advancedChunks`).
  /// Instead of keeping an empty entry chunk, we rewrite references to point directly to the common chunk
  /// and ensure proper symbol inclusion.
  pub(super) fn optimize_facade_dynamic_entry_chunks(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    index_splitting_info: &IndexSplittingInfo,
    input_base: &ArcStr,
    module_to_assigned: &mut IndexVec<ModuleIdx, bool>,
    temp_chunk_graph: &ChunkOptimizationGraph,
  ) {
    // Find empty dynamic entry chunks that should be merged with their target common chunks
    let (mut merge_entry_to_chunk, emitted_chunk_groups) =
      self.find_facade_chunk_merge_candidates(chunk_graph, temp_chunk_graph);

    if merge_entry_to_chunk.is_empty() && emitted_chunk_groups.is_empty() {
      return;
    }

    let runtime_module_idx = self.link_output.runtime.id();
    self.process_emitted_chunk_groups(chunk_graph, emitted_chunk_groups, &mut merge_entry_to_chunk);

    // Namespace symbols by default reference all exported symbols from the module.
    // To preserve dynamic import tree shaking, we should only include symbols that were actually used during the linking stage.
    // This ensures that including a namespace symbol doesn't inadvertently add unused exported symbols.
    for &entry_module in merge_entry_to_chunk.keys() {
      let wrap_kind = self.link_output.metas[entry_module].wrap_kind();
      let Some(module) = self.link_output.module_table[entry_module].as_normal_mut() else {
        continue;
      };
      // For CJS modules, we don't need to include `__exportAll` and the namespace symbols.
      // Instead, we should include the wrapper_ref (`require_xxx`), which will be handled
      // in the include_symbol call below.
      if !matches!(wrap_kind, WrapKind::Cjs) {
        // Filter in place to avoid cloning
        module.stmt_infos[StmtInfos::NAMESPACE_STMT_IDX].referenced_symbols.retain(
          |item| match item {
            rolldown_common::SymbolOrMemberExprRef::Symbol(symbol_ref) => {
              // module namespace symbol requires `__exportAll` runtime helper
              self.link_output.used_symbol_refs.contains(symbol_ref)
                || symbol_ref.owner == runtime_module_idx
            }
            rolldown_common::SymbolOrMemberExprRef::MemberExpr(_member_expr_ref) => true,
          },
        );
      }
    }

    let (mut stmt_info_included_vec, mut module_included_vec, mut module_namespace_reason_vec) =
      linking_metadata_vec_to_included_info(&mut self.link_output.metas);

    let runtime = &self.link_output.runtime;
    let context = &mut IncludeContext {
      modules: &self.link_output.module_table.modules,
      symbols: &self.link_output.symbol_db,
      is_included_vec: &mut stmt_info_included_vec,
      is_module_included_vec: &mut module_included_vec,
      tree_shaking: self.options.treeshake.is_some(),
      runtime_idx: self.link_output.runtime.id(),
      metas: &self.link_output.metas,
      used_symbol_refs: &mut self.link_output.used_symbol_refs,
      constant_symbol_map: &self.link_output.global_constant_symbol_map,
      options: self.options,
      normal_symbol_exports_chain_map: &self.link_output.normal_symbol_exports_chain_map,
      bailout_cjs_tree_shaking_modules: FxHashSet::default(),
      may_partial_namespace: false,
      module_namespace_included_reason: &mut module_namespace_reason_vec,
      inline_const_smart: self.options.optimization.is_inline_const_smart_mode(),
      json_module_none_self_reference_included_symbol: FxHashMap::default(),
    };

    let mut optimized_common_chunks = FxHashSet::default();

    let mut needs_export_all_runtime = false;
    for (&entry_module, &(from_chunk_idx, target_chunk_idx, elimination_reason)) in
      &merge_entry_to_chunk
    {
      // Point the entry module to related common chunk
      chunk_graph.entry_module_to_entry_chunk.remove(&entry_module);

      let Some(module) = context.modules[entry_module].as_normal() else {
        continue;
      };

      let wrap_kind = self.link_output.metas[entry_module].wrap_kind();

      chunk_graph.entry_module_to_entry_chunk.insert(entry_module, target_chunk_idx);
      let from_chunk = &chunk_graph.chunk_table[from_chunk_idx];
      let ChunkKind::EntryPoint { meta: chunk_meta, .. } = from_chunk.kind else {
        // We don't have any optimization to merge common chunks into other chunks.
        continue;
      };

      chunk_graph.post_chunk_optimization_operations.insert(from_chunk_idx, {
        let mut meta = PostChunkOptimizationOperation::Removed;
        meta.set(
          PostChunkOptimizationOperation::PreserveExports,
          chunk_meta.contains(ChunkMeta::EmittedChunk),
        );
        meta
      });

      // Track emitted chunks so their export names are preserved (not minified)
      if chunk_meta.contains(ChunkMeta::EmittedChunk) {
        chunk_graph
          .common_chunk_preserve_export_names_modules
          .entry(target_chunk_idx)
          .or_default()
          .insert(entry_module);
      }

      // If a chunk is not dynamically imported, we don't need to simulate a facade chunk.
      if !chunk_meta.contains(ChunkMeta::DynamicImported) {
        continue;
      }
      chunk_graph
        .common_chunk_exported_facade_chunk_namespace
        .entry(target_chunk_idx)
        .or_default()
        .insert(entry_module);

      // Add debug info about eliminated facade chunk to target chunk
      if self.options.experimental.is_attach_debug_info_full() || self.options.devtools {
        let eliminated_chunk_name = chunk_graph.chunk_table[from_chunk_idx]
          .name
          .as_ref()
          .map_or_else(|| "unnamed".to_string(), ArcStr::to_string);
        let module_stable_id = module.stable_id.to_string();
        chunk_graph.chunk_table[target_chunk_idx].debug_info.push(
          ChunkDebugInfo::EliminatedFacadeChunk {
            chunk_name: eliminated_chunk_name,
            entry_module_id: module_stable_id,
            reason: elimination_reason,
          },
        );
      }

      // For CJS modules, include the wrapper_ref (require_xxx) instead of namespace
      // and use ToEsm runtime helper instead of ExportAll
      if matches!(wrap_kind, WrapKind::Cjs | WrapKind::Esm) {
        if let Some(wrapper_ref) = self.link_output.metas[entry_module].wrapper_ref {
          include_symbol(context, wrapper_ref, SymbolIncludeReason::SimulatedFacadeChunk);
        }
        optimized_common_chunks.insert(target_chunk_idx);
      }
      if matches!(wrap_kind, WrapKind::Esm | WrapKind::None) {
        include_symbol(
          context,
          module.namespace_object_ref,
          SymbolIncludeReason::SimulatedFacadeChunk,
        );
        context.module_namespace_included_reason[entry_module]
          .insert(ModuleNamespaceIncludedReason::SimulateFacadeChunk);
        let target_chunk = &mut chunk_graph.chunk_table[target_chunk_idx];
        target_chunk.depended_runtime_helper.insert(RuntimeHelper::ExportAll);
        optimized_common_chunks.insert(target_chunk_idx);
        needs_export_all_runtime = true;
      }
    }

    if needs_export_all_runtime {
      include_runtime_symbol(context, runtime, RuntimeHelper::ExportAll);
    }

    // Ensure runtime module is properly assigned to chunk graph
    if chunk_graph.module_to_chunk[runtime_module_idx].is_none()
      && !optimized_common_chunks.is_empty()
    {
      // If only one common chunk was appended with dynamic entry module, we just put runtime module into that chunk.
      // Else create a new common chunk to store runtime module.
      let chunk_idx = match optimized_common_chunks.len() {
        1 => optimized_common_chunks.into_iter().next().unwrap(),
        _ => {
          let runtime_chunk = Chunk::new(
            Some("rolldown-runtime".into()),
            None,
            index_splitting_info[runtime_module_idx].bits.clone(),
            vec![],
            ChunkKind::Common,
            input_base.clone(),
            None,
          );
          chunk_graph.add_chunk(runtime_chunk)
        }
      };
      chunk_graph.add_module_to_chunk(
        runtime_module_idx,
        chunk_idx,
        self.link_output.metas[runtime_module_idx].depended_runtime_helper,
      );
      module_to_assigned[runtime_module_idx] = true;
    }

    // Restore the included info back to metas
    included_info_to_linking_metadata_vec(
      &mut self.link_output.metas,
      stmt_info_included_vec,
      &module_included_vec,
      &module_namespace_reason_vec,
    );
  }
}
