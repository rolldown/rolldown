use std::collections::VecDeque;

use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleIdx,
  ModuleNamespaceIncludedReason, PreserveEntrySignatures, RuntimeHelper, StmtInfoIdx, StmtInfos,
  UsedSymbolRefsBuilder, WrapKind, dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_utils::{BitSet, indexmap::FxIndexMap};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  stages::link_stage::{
    IncludeContext, SymbolIncludeReason, compute_body_demand_keys, include_runtime_symbol,
    include_symbol,
  },
  types::linking_metadata::{
    included_info_to_linking_metadata_vec, linking_metadata_vec_to_included_info,
  },
};

use super::{GenerateStage, code_splitting::IndexSplittingInfo};

struct ChunkAtom {
  modules: Vec<ModuleIdx>,
  dependent_entries: BitSet,
}

struct DynamicEntryAnalysis {
  dynamic_entry_indices: Vec<usize>,
  dynamic_entry_modules_by_entry: Vec<Option<ModuleIdx>>,
  dynamic_imports_by_entry: Vec<BitSet>,
  dynamically_dependent_entries_by_dynamic_entry: Vec<BitSet>,
}

enum ReducedEntriesDecision {
  Reject,
  Accept,
  /// Accept, but the surviving dynamic entry's chunk must publish the entry's simulated
  /// namespace and every `import()` of the entry must extract it. This is Rollup's
  /// `generateFacades` fallback (`Chunk.ts`): a dynamic entry that cannot cleanly represent
  /// its chunk gets its namespace object included and exported, and call sites append
  /// `.then(n => n.<alias>)`.
  AcceptWithNamespaceExtraction {
    chunk_idx: ChunkIdx,
    entry_module_idx: ModuleIdx,
  },
}

impl GenerateStage<'_> {
  pub(super) fn optimize_dynamic_entry_bits(
    &mut self,
    index_splitting_info: &mut IndexSplittingInfo,
    chunk_graph: &mut ChunkGraph,
    entries_len: u32,
    used_symbol_refs: &mut UsedSymbolRefsBuilder,
  ) {
    let DynamicEntryAnalysis {
      dynamic_entry_indices,
      dynamic_entry_modules_by_entry,
      dynamic_imports_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
    } = self.analyze_dynamic_entries(index_splitting_info, entries_len);
    if dynamic_entry_indices.is_empty() {
      return;
    }

    let mut atoms = self.group_modules_by_dependent_entries(index_splitting_info);
    if atoms.is_empty() {
      return;
    }
    let module_to_atom_idx = self.compute_module_to_atom_idx(&atoms);
    let atom_dependencies = self.compute_atom_dependencies(&atoms, &module_to_atom_idx);

    let static_dependency_atoms_by_entry =
      Self::compute_static_dependency_atoms_by_entry(entries_len as usize, &atoms);
    let already_loaded_atoms_by_entry = Self::compute_already_loaded_atoms_by_entry(
      &static_dependency_atoms_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
      &dynamic_imports_by_entry,
      &dynamic_entry_indices,
      atoms.len(),
    );

    let mut changed = false;
    let mut extraction_entries: FxIndexMap<ChunkIdx, ModuleIdx> = FxIndexMap::default();
    for atom_idx in 0..atoms.len() {
      let original_dependent_entries = atoms[atom_idx].dependent_entries.clone();
      let dependent_entries = atoms[atom_idx].dependent_entries.index_of_one().collect::<Vec<_>>();
      let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
      for entry_idx in dependent_entries {
        if already_loaded_atoms_by_entry[entry_idx as usize].has_bit(atom_bit) {
          atoms[atom_idx].dependent_entries.clear_bit(entry_idx);
        }
      }
      let decision = if atoms[atom_idx].dependent_entries == original_dependent_entries {
        ReducedEntriesDecision::Reject
      } else {
        self.reduced_dependent_entries_decision(
          &atoms[atom_idx],
          &original_dependent_entries,
          &atoms[atom_idx].dependent_entries,
          chunk_graph,
          &dynamic_entry_modules_by_entry,
        )
      };
      if !matches!(decision, ReducedEntriesDecision::Reject)
        && !Self::reduced_atom_graph_has_static_cycle(&atoms, &atom_dependencies)
      {
        changed = true;
        if let ReducedEntriesDecision::AcceptWithNamespaceExtraction {
          chunk_idx,
          entry_module_idx,
        } = decision
        {
          extraction_entries.insert(chunk_idx, entry_module_idx);
        }
      } else {
        atoms[atom_idx].dependent_entries = original_dependent_entries;
      }
    }

    if !changed {
      return;
    }

    for atom in atoms {
      let share_count = atom.dependent_entries.bit_count();
      for module_idx in atom.modules {
        index_splitting_info[module_idx].bits = atom.dependent_entries.clone();
        index_splitting_info[module_idx].share_count = share_count;
      }
    }

    if !extraction_entries.is_empty() {
      self.export_simulated_namespaces_for_dynamic_entries(
        chunk_graph,
        &extraction_entries,
        used_symbol_refs,
      );
    }
  }

  fn reduced_dependent_entries_decision(
    &self,
    atom: &ChunkAtom,
    original_dependent_entries: &BitSet,
    dependent_entries: &BitSet,
    chunk_graph: &ChunkGraph,
    dynamic_entry_modules_by_entry: &[Option<ModuleIdx>],
  ) -> ReducedEntriesDecision {
    let bit_count = dependent_entries.bit_count();
    if bit_count != 1 {
      return if bit_count > 1 {
        ReducedEntriesDecision::Accept
      } else {
        ReducedEntriesDecision::Reject
      };
    }

    let Some(entry_bit) = dependent_entries.index_of_one().next() else {
      return ReducedEntriesDecision::Reject;
    };
    let chunk_idx = ChunkIdx::from_raw(entry_bit);
    let Some(chunk) = chunk_graph.chunk_table.get(chunk_idx) else {
      return ReducedEntriesDecision::Reject;
    };

    let can_merge_without_changing_entry_signature =
      self.can_merge_without_changing_entry_signature(chunk, &atom.modules);
    let is_runtime_only_atom = self.is_runtime_only_atom(atom);
    let removed_entries_are_dynamic_entry_modules = Self::removed_entries_are_dynamic_entry_modules(
      atom,
      original_dependent_entries,
      dependent_entries,
      dynamic_entry_modules_by_entry,
    );

    if chunk.is_async_entry() {
      if can_merge_without_changing_entry_signature
        || is_runtime_only_atom
        || removed_entries_are_dynamic_entry_modules
        || self.dynamic_entry_extra_exports_are_unobservable(chunk)
      {
        return ReducedEntriesDecision::Accept;
      }
      if self.dynamic_entry_namespace_extraction_allowed(chunk)
        && let ChunkKind::EntryPoint { module: entry_module_idx, .. } = &chunk.kind
      {
        return ReducedEntriesDecision::AcceptWithNamespaceExtraction {
          chunk_idx,
          entry_module_idx: *entry_module_idx,
        };
      }
      return ReducedEntriesDecision::Reject;
    }

    if !matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::Strict))
      || can_merge_without_changing_entry_signature
      || is_runtime_only_atom
      || removed_entries_are_dynamic_entry_modules
    {
      ReducedEntriesDecision::Accept
    } else {
      ReducedEntriesDecision::Reject
    }
  }

  fn is_runtime_only_atom(&self, atom: &ChunkAtom) -> bool {
    atom.modules.len() == 1 && atom.modules[0] == self.link_output.runtime.id()
  }

  /// Accept an atom with extra exports when they can never be observed through the dynamic
  /// entry's namespace (#10263). This holds when every `import()` of the entry only reads a
  /// statically known set of names (`DynamicImportExportsUsage::Partial`) and each of those
  /// names resolves to one of the entry's own non-ambiguous exports: the chunk keeps exporting
  /// the used names, and the generated names of the atom's exports are deconflicted against
  /// them, so no recorded read can reach a merged binding.
  ///
  /// Restricted to pure dynamic entries — a user-defined or emitted entry's chunk file can be
  /// loaded directly at runtime, so its observable namespace is not limited to the recorded
  /// `import()` usage. Reads of names the entry does not export (`undefined` in source
  /// semantics) also reject the merge, because a generated export name could later shadow them.
  fn dynamic_entry_extra_exports_are_unobservable(&self, chunk: &Chunk) -> bool {
    let ChunkKind::EntryPoint { meta, module: entry_module_idx, .. } = &chunk.kind else {
      return false;
    };
    if *meta != ChunkMeta::DynamicImported {
      return false;
    }
    match self.link_output.dynamic_import_exports_usage_map.get(entry_module_idx) {
      Some(DynamicImportExportsUsage::Partial(used_names)) => {
        let entry_meta = &self.link_output.metas[*entry_module_idx];
        used_names
          .iter()
          .all(|used_name| entry_meta.canonical_exports(false).any(|(name, _)| name == used_name))
      }
      _ => false,
    }
  }

  /// Extraction fallback for atoms whose extra exports are observable: keep the merge and
  /// hide the extra exports the way Rollup does (`Chunk.ts` `generateFacades`). Rolldown
  /// already owns that machinery for eliminated facade chunks —
  /// `common_chunk_exported_facade_chunk_namespace` plus the finalizer's
  /// `rewrite_dynamic_import_for_merged_entry` — so the pass only needs to opt the surviving
  /// entry chunk into it.
  ///
  /// Extraction is refused when the simulated namespace cannot reproduce the entry's
  /// observable interface: dynamic exports (CJS `export *`, a star chain reaching an
  /// external) are format-dependent merges the namespace object cannot mirror, and a
  /// statically known `then` export makes the chunk namespace thenable — `import()` of the
  /// chunk would assimilate through it, handing the extraction callback the assimilated
  /// value instead of the chunk namespace. Restricted to pure dynamic entries for the same
  /// reason as the fast path above.
  fn dynamic_entry_namespace_extraction_allowed(&self, chunk: &Chunk) -> bool {
    let ChunkKind::EntryPoint { meta, module: entry_module_idx, .. } = &chunk.kind else {
      return false;
    };
    if *meta != ChunkMeta::DynamicImported {
      return false;
    }
    if self.link_output.module_table[*entry_module_idx].as_normal().is_none() {
      return false;
    }
    let entry_meta = &self.link_output.metas[*entry_module_idx];
    !entry_meta.has_dynamic_exports
      && !entry_meta.canonical_exports(true).any(|(name, _)| name.as_str() == "then")
  }

  /// Opt each surviving dynamic entry chunk into the simulated-facade machinery: narrow the
  /// entry's namespace statement to link-retained getters, include the namespace/wrapper
  /// symbols, register the chunk in `common_chunk_exported_facade_chunk_namespace`, and
  /// record the `__exportAll` runtime demand. This mirrors the recipe
  /// `optimize_facade_entry_chunks` applies to eliminated facade chunks, and runs before the
  /// standalone runtime chunk is extracted so the new helper demand participates in normal
  /// runtime placement.
  fn export_simulated_namespaces_for_dynamic_entries(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    extraction_entries: &FxIndexMap<ChunkIdx, ModuleIdx>,
    used_symbol_refs: &mut UsedSymbolRefsBuilder,
  ) {
    let runtime_module_idx = self.link_output.runtime.id();
    for &entry_module_idx in extraction_entries.values() {
      // Same narrowing as facade elimination: the simulated namespace exposes only the
      // getters link-time `import()` consumers retained, plus runtime helpers.
      if !matches!(self.link_output.metas[entry_module_idx].wrap_kind(), WrapKind::Cjs) {
        self.link_output.stmt_infos[entry_module_idx][StmtInfos::NAMESPACE_STMT_IDX]
          .referenced_symbols
          .retain(|item| match item {
            rolldown_common::SymbolOrMemberExprRef::Symbol(symbol_ref) => {
              used_symbol_refs.contains(symbol_ref) || symbol_ref.owner == runtime_module_idx
            }
            rolldown_common::SymbolOrMemberExprRef::MemberExpr(_) => true,
          });
      }
    }

    let (mut stmt_info_included_vec, mut module_included_vec, mut module_namespace_reason_vec) =
      linking_metadata_vec_to_included_info(&mut self.link_output.metas);
    let body_demand_keys = compute_body_demand_keys(
      &self.link_output.module_table.modules,
      &self.link_output.stmt_infos,
      &self.link_output.symbol_db,
      self.options.treeshake.is_some(),
      &self.link_output.user_defined_entry_modules,
    );
    let runtime = &self.link_output.runtime;
    let context = &mut IncludeContext {
      modules: &self.link_output.module_table.modules,
      stmt_infos: &self.link_output.stmt_infos,
      symbols: &self.link_output.symbol_db,
      is_included_vec: &mut stmt_info_included_vec,
      is_module_included_vec: &mut module_included_vec,
      tree_shaking: self.options.treeshake.is_some(),
      runtime_idx: self.link_output.runtime.id(),
      metas: &self.link_output.metas,
      used_symbol_refs,
      used_external_symbols: &mut self.link_output.used_external_symbols,
      constant_symbol_map: &self.link_output.global_constant_symbol_map,
      options: self.options,
      normal_symbol_exports_chain_map: &self.link_output.normal_symbol_exports_chain_map,
      bailout_cjs_tree_shaking_modules: FxHashSet::default(),
      external_importer_modes: FxHashMap::default(),
      module_inclusion_changed: false,
      module_namespace_included_reason: &mut module_namespace_reason_vec,
      inline_const_smart: self.options.optimization.is_inline_const_smart_mode(),
      json_module_none_self_reference_included_symbol: FxHashMap::default(),
      entry_module_idxs: &self.link_output.user_defined_entry_modules,
      body_demand_keys: &body_demand_keys,
      body_demand_swept: FxHashSet::default(),
      pending: Vec::new(),
    };

    let mut needs_export_all_helper = false;
    for (&chunk_idx, &entry_module_idx) in extraction_entries {
      chunk_graph
        .common_chunk_exported_facade_chunk_namespace
        .entry(chunk_idx)
        .or_default()
        .insert(entry_module_idx);

      let Some(module) = context.modules[entry_module_idx].as_normal() else {
        continue;
      };
      let wrap_kind = context.metas[entry_module_idx].wrap_kind();
      if matches!(wrap_kind, WrapKind::Cjs | WrapKind::Esm)
        && let Some(wrapper_ref) = context.metas[entry_module_idx].wrapper_ref
      {
        include_symbol(context, wrapper_ref, SymbolIncludeReason::SimulatedFacadeChunk);
      }
      if matches!(wrap_kind, WrapKind::Esm | WrapKind::None) {
        include_symbol(
          context,
          module.namespace_object_ref,
          SymbolIncludeReason::SimulatedFacadeChunk,
        );
        context.module_namespace_included_reason[entry_module_idx]
          .insert(ModuleNamespaceIncludedReason::SimulateFacadeChunk);
        chunk_graph.chunk_table[chunk_idx].depended_runtime_helper.insert(RuntimeHelper::ExportAll);
        needs_export_all_helper = true;
      }
    }
    if needs_export_all_helper {
      include_runtime_symbol(context, runtime, RuntimeHelper::ExportAll);
    }

    included_info_to_linking_metadata_vec(
      &mut self.link_output.metas,
      stmt_info_included_vec,
      &module_included_vec,
      &module_namespace_reason_vec,
    );
  }

  fn removed_entries_are_dynamic_entry_modules(
    atom: &ChunkAtom,
    original_dependent_entries: &BitSet,
    dependent_entries: &BitSet,
    dynamic_entry_modules_by_entry: &[Option<ModuleIdx>],
  ) -> bool {
    let mut has_removed_dynamic_entry = false;
    for removed_entry_idx in
      original_dependent_entries.index_of_one().filter(|idx| !dependent_entries.has_bit(*idx))
    {
      let Some(dynamic_entry_module_idx) =
        dynamic_entry_modules_by_entry.get(removed_entry_idx as usize).copied().flatten()
      else {
        return false;
      };
      has_removed_dynamic_entry = true;
      if !atom.modules.contains(&dynamic_entry_module_idx) {
        return false;
      }
    }
    has_removed_dynamic_entry
  }

  fn compute_module_to_atom_idx(&self, atoms: &[ChunkAtom]) -> IndexVec<ModuleIdx, Option<usize>> {
    let mut module_to_atom_idx = index_vec![None; self.link_output.module_table.modules.len()];
    for (atom_idx, atom) in atoms.iter().enumerate() {
      for &module_idx in &atom.modules {
        module_to_atom_idx[module_idx] = Some(atom_idx);
      }
    }
    module_to_atom_idx
  }

  fn compute_atom_dependencies(
    &self,
    atoms: &[ChunkAtom],
    module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
  ) -> Vec<Vec<usize>> {
    atoms
      .iter()
      .enumerate()
      .map(|(atom_idx, atom)| {
        let mut dependencies = FxHashSet::default();
        for &module_idx in &atom.modules {
          for &dep_module_idx in &self.link_output.metas[module_idx].dependencies {
            let Some(dep_atom_idx) = module_to_atom_idx[dep_module_idx] else {
              continue;
            };
            if dep_atom_idx != atom_idx {
              dependencies.insert(dep_atom_idx);
            }
          }
        }
        dependencies.into_iter().collect()
      })
      .collect()
  }

  fn reduced_atom_graph_has_static_cycle(
    atoms: &[ChunkAtom],
    atom_dependencies: &[Vec<usize>],
  ) -> bool {
    let mut chunk_idx_by_bits = FxHashMap::default();
    let mut atom_to_chunk = Vec::with_capacity(atoms.len());
    for atom in atoms {
      let next_chunk_idx = chunk_idx_by_bits.len();
      let chunk_idx = match chunk_idx_by_bits.entry(atom.dependent_entries.clone()) {
        std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
        std::collections::hash_map::Entry::Vacant(vacant) => {
          vacant.insert(next_chunk_idx);
          next_chunk_idx
        }
      };
      atom_to_chunk.push(chunk_idx);
    }

    let mut chunk_dependencies = vec![FxHashSet::default(); chunk_idx_by_bits.len()];
    for (atom_idx, dependencies) in atom_dependencies.iter().enumerate() {
      let from_chunk_idx = atom_to_chunk[atom_idx];
      for &dep_atom_idx in dependencies {
        let to_chunk_idx = atom_to_chunk[dep_atom_idx];
        if from_chunk_idx != to_chunk_idx {
          chunk_dependencies[from_chunk_idx].insert(to_chunk_idx);
        }
      }
    }

    Self::chunk_dependency_graph_has_cycle(&chunk_dependencies)
  }

  fn chunk_dependency_graph_has_cycle(chunk_dependencies: &[FxHashSet<usize>]) -> bool {
    let mut state = vec![0_u8; chunk_dependencies.len()];
    for start_chunk_idx in 0..chunk_dependencies.len() {
      if state[start_chunk_idx] != 0 {
        continue;
      }

      let mut stack = vec![(start_chunk_idx, false)];
      while let Some((chunk_idx, exiting)) = stack.pop() {
        if exiting {
          state[chunk_idx] = 2;
          continue;
        }

        match state[chunk_idx] {
          1 => return true,
          2 => continue,
          _ => {}
        }

        state[chunk_idx] = 1;
        stack.push((chunk_idx, true));
        for &dep_chunk_idx in &chunk_dependencies[chunk_idx] {
          match state[dep_chunk_idx] {
            1 => return true,
            0 => stack.push((dep_chunk_idx, false)),
            _ => {}
          }
        }
      }
    }

    false
  }

  fn analyze_dynamic_entries(
    &self,
    index_splitting_info: &IndexSplittingInfo,
    entries_len: u32,
  ) -> DynamicEntryAnalysis {
    let entries_count = entries_len as usize;
    let mut dynamic_entry_indices = vec![];
    let mut dynamic_entry_modules_by_entry = vec![None; entries_count];
    let mut dynamic_imports_by_entry = vec![BitSet::new(entries_len); entries_count];
    let mut dynamically_dependent_entries_by_dynamic_entry =
      vec![BitSet::new(entries_len); entries_count];

    for (dynamic_entry_idx, (dynamic_entry_module_idx, entry_point)) in self
      .link_output
      .entries
      .iter()
      .flat_map(|(&idx, entries)| entries.iter().map(move |entry| (idx, entry)))
      .enumerate()
    {
      if !entry_point.kind.is_dynamic_import() {
        continue;
      }

      dynamic_entry_indices.push(dynamic_entry_idx);
      dynamic_entry_modules_by_entry[dynamic_entry_idx] = Some(dynamic_entry_module_idx);
      let dynamic_entry_bit: u32 =
        dynamic_entry_idx.try_into().expect("Too many entries, u32 overflowed.");

      for (importer_idx, stmt_info_idx, _node_id, import_record_idx) in
        &entry_point.related_stmt_infos
      {
        if !self.is_included_dynamic_import_record(
          *importer_idx,
          *stmt_info_idx,
          *import_record_idx,
          dynamic_entry_module_idx,
        ) {
          continue;
        }

        for importer_entry_idx in index_splitting_info[*importer_idx].bits.index_of_one() {
          dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx]
            .set_bit(importer_entry_idx);
          dynamic_imports_by_entry[importer_entry_idx as usize].set_bit(dynamic_entry_bit);
        }
      }
    }

    DynamicEntryAnalysis {
      dynamic_entry_indices,
      dynamic_entry_modules_by_entry,
      dynamic_imports_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
    }
  }

  fn is_included_dynamic_import_record(
    &self,
    importer_idx: ModuleIdx,
    stmt_info_idx: StmtInfoIdx,
    import_record_idx: ImportRecordIdx,
    dynamic_entry_module_idx: ModuleIdx,
  ) -> bool {
    if !self.link_output.metas[importer_idx].stmt_info_included.has_bit(stmt_info_idx) {
      return false;
    }

    let Some(importer) = self.link_output.module_table[importer_idx].as_normal() else {
      return false;
    };
    let Some(import_record) = importer.import_records.get(import_record_idx) else {
      return false;
    };

    import_record.kind == ImportKind::DynamicImport
      && import_record.resolved_module == Some(dynamic_entry_module_idx)
      && !import_record.meta.contains(ImportRecordMeta::DeadDynamicImport)
  }

  fn group_modules_by_dependent_entries(
    &self,
    index_splitting_info: &IndexSplittingInfo,
  ) -> Vec<ChunkAtom> {
    let mut atoms = vec![];
    let mut bits_to_atom_idx = FxHashMap::default();
    for module_idx in &self.link_output.sorted_modules {
      let Some(normal_module) = self.link_output.module_table[*module_idx].as_normal() else {
        continue;
      };
      if !self.link_output.metas[normal_module.idx].is_included {
        continue;
      }

      let bits = &index_splitting_info[normal_module.idx].bits;
      if bits.is_empty() {
        continue;
      }

      let atom_idx = match bits_to_atom_idx.entry(bits.clone()) {
        std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
        std::collections::hash_map::Entry::Vacant(vacant) => {
          let atom_idx = atoms.len();
          atoms.push(ChunkAtom { modules: vec![], dependent_entries: bits.clone() });
          *vacant.insert(atom_idx)
        }
      };
      atoms[atom_idx].modules.push(normal_module.idx);
    }
    atoms
  }

  fn compute_static_dependency_atoms_by_entry(
    entries_count: usize,
    atoms: &[ChunkAtom],
  ) -> Vec<BitSet> {
    let atom_count: u32 = atoms.len().try_into().expect("Too many atoms, u32 overflowed.");
    let mut static_dependency_atoms_by_entry = vec![BitSet::new(atom_count); entries_count];
    for (atom_idx, atom) in atoms.iter().enumerate() {
      let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
      for entry_idx in atom.dependent_entries.index_of_one() {
        static_dependency_atoms_by_entry[entry_idx as usize].set_bit(atom_bit);
      }
    }
    static_dependency_atoms_by_entry
  }

  fn compute_already_loaded_atoms_by_entry(
    static_dependency_atoms_by_entry: &[BitSet],
    mut dynamically_dependent_entries_by_dynamic_entry: Vec<BitSet>,
    dynamic_imports_by_entry: &[BitSet],
    dynamic_entry_indices: &[usize],
    atom_count: usize,
  ) -> Vec<BitSet> {
    let entries_count = static_dependency_atoms_by_entry.len();
    let atom_count: u32 = atom_count.try_into().expect("Too many atoms, u32 overflowed.");
    let mut is_dynamic_entry = vec![false; entries_count];
    for &dynamic_entry_idx in dynamic_entry_indices {
      is_dynamic_entry[dynamic_entry_idx] = true;
    }

    let mut already_loaded_atoms_by_entry = is_dynamic_entry
      .iter()
      .map(|is_dynamic| if *is_dynamic { BitSet::all(atom_count) } else { BitSet::new(atom_count) })
      .collect::<Vec<_>>();

    let mut queued = vec![false; entries_count];
    let mut queue = VecDeque::new();
    for &dynamic_entry_idx in dynamic_entry_indices {
      if !dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx].is_empty() {
        queued[dynamic_entry_idx] = true;
        queue.push_back(dynamic_entry_idx);
      }
    }

    while let Some(dynamic_entry_idx) = queue.pop_front() {
      queued[dynamic_entry_idx] = false;
      let known_loaded_atoms = already_loaded_atoms_by_entry[dynamic_entry_idx].clone();
      let mut updated_loaded_atoms = known_loaded_atoms.clone();

      for importer_entry_idx in
        dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx].index_of_one()
      {
        let importer_entry_idx = importer_entry_idx as usize;
        let mut importer_loaded_atoms =
          static_dependency_atoms_by_entry[importer_entry_idx].clone();
        importer_loaded_atoms.union(&already_loaded_atoms_by_entry[importer_entry_idx]);
        updated_loaded_atoms.intersect(&importer_loaded_atoms);
      }

      if updated_loaded_atoms == known_loaded_atoms {
        continue;
      }

      already_loaded_atoms_by_entry[dynamic_entry_idx] = updated_loaded_atoms;
      let dynamic_entry_bit: u32 =
        dynamic_entry_idx.try_into().expect("Too many entries, u32 overflowed.");
      for next_dynamic_entry_idx in dynamic_imports_by_entry[dynamic_entry_idx].index_of_one() {
        let next_dynamic_entry_idx = next_dynamic_entry_idx as usize;
        dynamically_dependent_entries_by_dynamic_entry[next_dynamic_entry_idx]
          .set_bit(dynamic_entry_bit);
        if !queued[next_dynamic_entry_idx] {
          queued[next_dynamic_entry_idx] = true;
          queue.push_back(next_dynamic_entry_idx);
        }
      }
    }

    already_loaded_atoms_by_entry
  }
}
