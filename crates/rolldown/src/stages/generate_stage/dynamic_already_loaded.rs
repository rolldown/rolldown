use std::collections::VecDeque;

use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ChunkIdx, ChunkKind, ChunkMeta, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleIdx,
  PreserveEntrySignatures, RuntimeHelper, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef,
  UsedSymbolRefsBuilder, UsedSymbolRefsView, WrapKind,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;
use crate::esm_init_obligations::collect_entry_reexported_wrapper_inits;

use super::{
  GenerateStage, code_splitting::IndexSplittingInfo,
  simulated_facade_inclusion::include_simulated_facade_namespace,
};

struct ChunkAtom {
  modules: Vec<ModuleIdx>,
  dependent_entries: BitSet,
}

/// Atom-level edge graphs for the pass's two consumers. `reachability` under-approximates the
/// emitted chunk imports (safe to miss an edge, never to invent one); `cycle` over-approximates
/// them (safe to invent an edge, never to miss one). See `compute_atom_dependencies`.
struct AtomDependencyGraphs {
  reachability: Vec<Vec<usize>>,
  cycle: Vec<Vec<usize>>,
}

struct DynamicEntryAnalysis {
  dynamic_entry_indices: Vec<usize>,
  dynamic_entry_modules_by_entry: Vec<Option<ModuleIdx>>,
  dynamic_imports_by_entry: Vec<BitSet>,
  dynamically_dependent_entries_by_dynamic_entry: Vec<BitSet>,
}

enum ReducedEntriesAction {
  Avoid,
  Apply,
  // The atom can be added to the dynamic entry chunk, but it needs care to preserve
  // its namespace due to it being dynamically imported.
  ApplyWithNamespaceExtraction { entry_chunk_idx: ChunkIdx, entry_module_idx: ModuleIdx },
}

impl ReducedEntriesAction {
  fn apply_if(apply: bool) -> Self {
    if apply { ReducedEntriesAction::Apply } else { ReducedEntriesAction::Avoid }
  }
}

impl GenerateStage<'_> {
  pub(super) fn optimize_dynamic_entry_bits(
    &mut self,
    index_splitting_info: &mut IndexSplittingInfo,
    chunk_graph: &mut ChunkGraph,
    entries_len: u32,
    used_symbol_refs_builder: &mut UsedSymbolRefsBuilder,
  ) {
    let mut namespace_extractions = FxHashSet::default();
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
    let atom_dependencies =
      self.compute_atom_dependencies(&atoms, &module_to_atom_idx, used_symbol_refs_builder);

    let static_dependency_atoms_by_entry = self.compute_static_dependency_atoms_by_entry(
      entries_len as usize,
      &atoms,
      &atom_dependencies.reachability,
      &module_to_atom_idx,
    );
    let already_loaded_atoms_by_entry = Self::compute_already_loaded_atoms_by_entry(
      &static_dependency_atoms_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
      &dynamic_imports_by_entry,
      &dynamic_entry_indices,
      atoms.len(),
    );

    let mut changed = false;
    for atom_idx in 0..atoms.len() {
      let original_dependent_entries = atoms[atom_idx].dependent_entries.clone();
      let dependent_entries = atoms[atom_idx].dependent_entries.index_of_one().collect::<Vec<_>>();
      let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
      for entry_idx in dependent_entries {
        if already_loaded_atoms_by_entry[entry_idx as usize].has_bit(atom_bit) {
          atoms[atom_idx].dependent_entries.clear_bit(entry_idx);
        }
      }
      let action = if atoms[atom_idx].dependent_entries == original_dependent_entries {
        ReducedEntriesAction::Avoid
      } else {
        self.can_use_reduced_dependent_entries(
          &atoms[atom_idx],
          &original_dependent_entries,
          &atoms[atom_idx].dependent_entries,
          chunk_graph,
          &dynamic_entry_modules_by_entry,
        )
      };
      if !matches!(action, ReducedEntriesAction::Avoid)
        && !Self::reduced_atom_graph_has_static_cycle(&atoms, &atom_dependencies.cycle)
      {
        changed = true;
        if let ReducedEntriesAction::ApplyWithNamespaceExtraction {
          entry_chunk_idx,
          entry_module_idx,
        } = action
        {
          namespace_extractions.insert((entry_chunk_idx, entry_module_idx));
        }
      } else {
        atoms[atom_idx].dependent_entries = original_dependent_entries;
      }
    }

    if !changed {
      debug_assert!(namespace_extractions.is_empty());
      return;
    }

    for atom in atoms {
      let share_count = atom.dependent_entries.bit_count();
      for module_idx in atom.modules {
        index_splitting_info[module_idx].bits = atom.dependent_entries.clone();
        index_splitting_info[module_idx].share_count = share_count;
      }
    }

    self.apply_dynamic_entry_namespace_extractions(
      chunk_graph,
      &namespace_extractions,
      used_symbol_refs_builder,
    );
  }

  fn can_use_reduced_dependent_entries(
    &self,
    atom: &ChunkAtom,
    original_dependent_entries: &BitSet,
    dependent_entries: &BitSet,
    chunk_graph: &ChunkGraph,
    dynamic_entry_modules_by_entry: &[Option<ModuleIdx>],
  ) -> ReducedEntriesAction {
    let bit_count = dependent_entries.bit_count();
    if bit_count != 1 {
      return ReducedEntriesAction::apply_if(bit_count > 1);
    }

    let Some(entry_bit) = dependent_entries.index_of_one().next() else {
      return ReducedEntriesAction::Avoid;
    };
    let entry_chunk_idx = ChunkIdx::from_raw(entry_bit);
    let Some(chunk) = chunk_graph.chunk_table.get(entry_chunk_idx) else {
      return ReducedEntriesAction::Avoid;
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

    if can_merge_without_changing_entry_signature
      || is_runtime_only_atom
      || removed_entries_are_dynamic_entry_modules
    {
      return ReducedEntriesAction::Apply;
    }

    // If a chunk is a dynamic entry point, we may still inline into it by taking care of the namespace.
    if let ChunkKind::EntryPoint { meta, module: entry_module_idx, .. } = chunk.kind
      && meta == ChunkMeta::DynamicImported
    {
      if self.dynamic_entry_partial_usage_allows_plain_merge(entry_module_idx) {
        return ReducedEntriesAction::Apply;
      }
      if self.dynamic_entry_supports_namespace_extraction(entry_module_idx) {
        return ReducedEntriesAction::ApplyWithNamespaceExtraction {
          entry_chunk_idx,
          entry_module_idx,
        };
      }
      return ReducedEntriesAction::Avoid;
    }

    ReducedEntriesAction::apply_if(
      !chunk.is_async_entry()
        && !matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::Strict)),
    )
  }

  /// If all the used exports of the dynamic import are statically known and actual exports
  /// of the module, code will never observe the extra exports and we can avoid the extra
  /// indirection of exporting a synthetic namespace.
  fn dynamic_entry_partial_usage_allows_plain_merge(&self, entry_module_idx: ModuleIdx) -> bool {
    let Some(DynamicImportExportsUsage::Partial(used)) =
      self.link_output.dynamic_import_exports_usage_map.get(&entry_module_idx)
    else {
      return false;
    };
    let resolved_exports = &self.link_output.metas[entry_module_idx].resolved_exports;
    used.iter().all(|name| resolved_exports.contains_key(name))
  }

  /// Whether `import()` of this dynamic entry can be preserved by exporting the
  /// entry's namespace object from its chunk and rewriting every dynamic importer to
  /// `.then((n) => n.<ns>)` (see `rewrite_dynamic_import_for_merged_entry`).
  fn dynamic_entry_supports_namespace_extraction(&self, entry_module_idx: ModuleIdx) -> bool {
    if self.link_output.module_table[entry_module_idx].as_normal().is_none() {
      return false;
    }

    // `import()` assimilates a namespace that carries a callable `then`. The extraction callback
    // would then get whatever that `then` returns, not the namespace. Only the export name can do
    // this. A local symbol named `then` that leaves under an alias is harmless, because the alias
    // is what lands in the namespace.
    //
    // No other export name can become `then`. The minified generator skips it, and
    // `ConflictResolver` reserves it up front. So an internal export named `then` deconflicts to
    // `then$1`. See `THENABLE_HAZARD_EXPORT_NAME` in compute_cross_chunk_links.rs.
    if self.link_output.metas[entry_module_idx]
      .resolved_exports
      .keys()
      .any(|name| name.as_str() == "then")
    {
      return false;
    }

    // If the entry has dynamic exports through `export * from`, it _might_
    // have a `then` export.
    !self.link_output.metas[entry_module_idx].has_dynamic_exports
  }

  /// Generates a definition for a fake module namespace for the inlined module, which is then
  /// exported. Dynamic importers of this atom from this chunk can then do `.then((n) => n.<ns>)`
  /// to get that namespace.
  fn apply_dynamic_entry_namespace_extractions(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    namespace_extractions: &FxHashSet<(ChunkIdx, ModuleIdx)>,
    used_symbol_refs_builder: &mut UsedSymbolRefsBuilder,
  ) {
    if namespace_extractions.is_empty() {
      return;
    }

    for &(_, entry_module_idx) in namespace_extractions {
      self.narrow_namespace_stmt_to_used_symbols(entry_module_idx, used_symbol_refs_builder);
    }

    self.replay_link_stage_inclusion(used_symbol_refs_builder, |context| {
      let mut needs_export_all_helper = false;
      for &(entry_chunk_idx, entry_module_idx) in namespace_extractions {
        chunk_graph
          .common_chunk_exported_facade_chunk_namespace
          .entry(entry_chunk_idx)
          .or_default()
          .insert(entry_module_idx);

        if include_simulated_facade_namespace(context, entry_module_idx) {
          chunk_graph.chunk_table[entry_chunk_idx]
            .depended_runtime_helper
            .insert(RuntimeHelper::ExportAll);
          needs_export_all_helper = true;
        }
      }
      needs_export_all_helper
    });
  }

  fn is_runtime_only_atom(&self, atom: &ChunkAtom) -> bool {
    atom.modules.len() == 1 && atom.modules[0] == self.link_output.runtime.id()
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

  // See internal-docs/code-splitting/implementation.md#dynamic-already-loaded-analysis.
  //
  // The two graphs serve the pass's two consumers, whose safe approximations point in opposite
  // directions. `reachability` must only contain edges the emitted chunks really have: its
  // entry-export service edges are liveness-gated and attached only when the entry module is
  // hosted by its own entry chunk, because emission hangs those imports on the entry (facade)
  // chunk, not on whichever shared chunk hosts the module. `cycle` must contain every edge the
  // emitted chunks might have: its service edges are attached to the hosting atom regardless,
  // with no liveness gate — `used_symbol_refs_builder` still grows after this pass (namespace-extraction
  // and facade-elimination replays), so an export dead here can be live at emission. A facade's
  // own service edges cannot close a cycle (facades have zero static in-degree), so attributing
  // them to the hosting atom only over-approximates. The price of the ungated edges is that a
  // dead entry re-export can conservatively veto a fold that would have been acyclic — a missed
  // optimization, accepted over trusting a liveness snapshot the replays may outgrow. The
  // liveness-growth immunity is scoped to service edges: the base prediction's ambiguous branch
  // (`referenced_symbol_owners`) still reads decision-time statement inclusion, a narrower
  // pre-existing skew the replays can also outgrow.
  fn compute_atom_dependencies(
    &self,
    atoms: &[ChunkAtom],
    module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
    used_symbol_refs_builder: &UsedSymbolRefsBuilder,
  ) -> AtomDependencyGraphs {
    let strict_execution_order = self.options.is_strict_execution_order_enabled();
    let flattened_entry_modules: Vec<ModuleIdx> = self
      .link_output
      .entries
      .iter()
      .flat_map(|(&idx, entries)| std::iter::repeat_n(idx, entries.len()))
      .collect();

    let mut reachability = Vec::with_capacity(atoms.len());
    let mut cycle = Vec::with_capacity(atoms.len());
    for (atom_idx, atom) in atoms.iter().enumerate() {
      let mut reachability_deps = FxHashSet::default();
      let mut cycle_deps = FxHashSet::default();
      let add = |dep_module_idx: ModuleIdx, dependencies: &mut FxHashSet<usize>| {
        if let Some(dep_atom_idx) = module_to_atom_idx[dep_module_idx]
          && dep_atom_idx != atom_idx
        {
          dependencies.insert(dep_atom_idx);
        }
      };
      for &module_idx in &atom.modules {
        if strict_execution_order {
          // Strict lowering can turn linked import records back into `init_*` imports.
          for &dep_module_idx in &self.link_output.metas[module_idx].dependencies {
            add(dep_module_idx, &mut reachability_deps);
            add(dep_module_idx, &mut cycle_deps);
          }
          continue;
        }
        for dep_module_idx in self.predicted_static_import_targets(module_idx) {
          add(dep_module_idx, &mut reachability_deps);
          add(dep_module_idx, &mut cycle_deps);
        }

        let mut service_targets = vec![];
        self.entry_export_service_targets(
          module_idx,
          used_symbol_refs_builder.view(),
          true,
          &mut service_targets,
        );
        for dep_module_idx in service_targets {
          add(dep_module_idx, &mut cycle_deps);
        }

        let entry_hosted_by_own_chunk = atom.dependent_entries.bit_count() == 1
          && atom.dependent_entries.index_of_one().next().is_some_and(|entry_bit| {
            flattened_entry_modules.get(entry_bit as usize) == Some(&module_idx)
          });
        if entry_hosted_by_own_chunk {
          let mut service_targets = vec![];
          self.entry_export_service_targets(
            module_idx,
            used_symbol_refs_builder.view(),
            false,
            &mut service_targets,
          );
          for dep_module_idx in service_targets {
            add(dep_module_idx, &mut reachability_deps);
          }
        }
      }
      reachability.push(reachability_deps.into_iter().collect());
      cycle.push(cycle_deps.into_iter().collect());
    }
    AtomDependencyGraphs { reachability, cycle }
  }

  /// Returns the targets that `compute_cross_chunk_links` will import for this module. Transitive
  /// side-effect dependencies behind a `sideEffects: false` barrel are retained only when an
  /// included symbol reference also requires them.
  pub(super) fn predicted_static_import_targets(&self, module_idx: ModuleIdx) -> Vec<ModuleIdx> {
    let meta = &self.link_output.metas[module_idx];
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return meta.load_dependencies.iter().copied().collect();
    };

    // A side-effectful runtime dependency has no import record.
    let side_effectful_runtime_idx =
      meta.has_side_effectful_runtime_dep.then(|| self.link_output.runtime.id());

    let direct_record_targets: FxHashSet<ModuleIdx> = module
      .import_records
      .iter()
      .filter(|rec| rec.kind != ImportKind::DynamicImport)
      .filter_map(|rec| rec.resolved_module)
      .collect();

    let mut ambiguous = vec![];
    let mut targets = Vec::with_capacity(meta.load_dependencies.len());
    for &dep_module_idx in &meta.load_dependencies {
      if !self.link_output.module_table[dep_module_idx].side_effects().has_side_effects()
        || side_effectful_runtime_idx == Some(dep_module_idx)
        || direct_record_targets.contains(&dep_module_idx)
      {
        targets.push(dep_module_idx);
      } else {
        ambiguous.push(dep_module_idx);
      }
    }
    if !ambiguous.is_empty() {
      let symbol_owners = self.referenced_symbol_owners(module_idx);
      targets.extend(ambiguous.into_iter().filter(|dep_idx| symbol_owners.contains(dep_idx)));
    }
    targets
  }

  /// An entry chunk imports the canonical symbol of every live resolved export it serves
  /// (`register_entry_export_depended_symbols`), even when no included statement of the entry
  /// references it — a re-export the entry never uses still becomes a real import of the owner's
  /// chunk. `load_dependencies` only carries the narrowed `referenced_symbols_by_entry_point_chunk`
  /// set, so these targets are derived separately from the full `resolved_exports`.
  ///
  /// With `assume_all_live: false` the liveness gate mirrors emission exactly: a dead export
  /// produces no import. `assume_all_live: true` skips the gate for consumers that need a stable
  /// over-approximation — `used_symbol_refs_builder` keeps growing after the fold's decisions, so a
  /// dead-here export can still emit an import later.
  pub(super) fn entry_export_service_targets(
    &self,
    module_idx: ModuleIdx,
    used_symbol_refs_view: UsedSymbolRefsView<'_>,
    assume_all_live: bool,
    targets: &mut Vec<ModuleIdx>,
  ) {
    if !self.link_output.entries.contains_key(&module_idx) {
      return;
    }
    let meta = &self.link_output.metas[module_idx];
    if matches!(meta.wrap_kind(), WrapKind::Cjs) {
      // A CJS entry is consumed through its wrapper, not through per-export symbols.
      return;
    }
    let symbol_db = &self.link_output.symbol_db;
    for resolved_export in meta.resolved_exports.values() {
      if resolved_export.came_from_commonjs {
        continue;
      }
      let served = symbol_db.canonical_ref_resolving_namespace(resolved_export.symbol_ref);
      let is_live = assume_all_live
        || if let Some(owner) = self.link_output.module_table[served.owner].as_normal()
          && owner.namespace_object_ref == served
        {
          self.link_output.metas[served.owner].namespace_included
        } else {
          used_symbol_refs_view.contains(&served)
        };
      if is_live {
        targets.push(served.owner);
      }
    }
    // A re-export of a wrapped ESM module additionally imports the module's `init_*` wrapper.
    for init in collect_entry_reexported_wrapper_inits(
      module_idx,
      meta,
      &self.link_output.metas,
      &self.link_output.module_table.modules,
      symbol_db,
      None,
    ) {
      if assume_all_live || used_symbol_refs_view.contains(&init.wrapper_ref) {
        targets.push(init.wrapper_ref.owner);
      }
    }
  }

  fn referenced_symbol_owners(&self, module_idx: ModuleIdx) -> FxHashSet<ModuleIdx> {
    let meta = &self.link_output.metas[module_idx];
    let mut owners = FxHashSet::default();
    let note = |symbol_ref: SymbolRef, owners: &mut FxHashSet<ModuleIdx>| {
      let canonical_ref = self.link_output.symbol_db.canonical_ref_for(symbol_ref);
      owners.insert(canonical_ref.owner);
      if let Some(ns) = &self.link_output.symbol_db.get(canonical_ref).namespace_alias {
        owners.insert(ns.namespace_ref.owner);
      }
    };
    for (symbol_ref, _) in &meta.referenced_symbols_by_entry_point_chunk {
      note(*symbol_ref, &mut owners);
    }
    for (stmt_idx, stmt_info) in self.link_output.stmt_infos[module_idx].iter_enumerated() {
      if !meta.stmt_info_included.has_bit(stmt_idx) {
        continue;
      }
      for reference_ref in &stmt_info.referenced_symbols {
        match reference_ref {
          SymbolOrMemberExprRef::Symbol(symbol_ref) => note(*symbol_ref, &mut owners),
          SymbolOrMemberExprRef::MemberExpr(member_expr) => {
            if let Some(symbol_ref) =
              member_expr.represent_symbol_ref(&meta.resolved_member_expr_refs)
            {
              note(symbol_ref, &mut owners);
            }
          }
        }
      }
    }
    owners
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
    &self,
    entries_count: usize,
    atoms: &[ChunkAtom],
    atom_dependencies: &[Vec<usize>],
    module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
  ) -> Vec<BitSet> {
    let atom_count: u32 = atoms.len().try_into().expect("Too many atoms, u32 overflowed.");
    let mut static_dependency_atoms_by_entry = vec![BitSet::new(atom_count); entries_count];

    if self.options.is_strict_execution_order_enabled() {
      // Conservative strict edges are safe for cycle detection, but cannot prove an atom is loaded.
      // Keep the `load_dependencies` reachability recorded in the dependent-entry bits.
      for (atom_idx, atom) in atoms.iter().enumerate() {
        let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
        for entry_idx in atom.dependent_entries.index_of_one() {
          static_dependency_atoms_by_entry[entry_idx as usize].set_bit(atom_bit);
        }
      }
      return static_dependency_atoms_by_entry;
    }

    for (entry_module_idx, loaded_atoms) in self
      .link_output
      .entries
      .iter()
      .flat_map(|(&idx, entries)| std::iter::repeat_n(idx, entries.len()))
      .zip(static_dependency_atoms_by_entry.iter_mut())
    {
      let Some(entry_atom_idx) = module_to_atom_idx[entry_module_idx] else {
        continue;
      };
      let mut stack = vec![entry_atom_idx];
      while let Some(atom_idx) = stack.pop() {
        let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
        if loaded_atoms.has_bit(atom_bit) {
          continue;
        }
        loaded_atoms.set_bit(atom_bit);
        stack.extend(atom_dependencies[atom_idx].iter().copied());
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
