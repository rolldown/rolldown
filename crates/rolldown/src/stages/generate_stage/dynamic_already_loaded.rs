use std::collections::VecDeque;

use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ChunkIdx, ChunkKind, ChunkMeta, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta,
  Module, ModuleIdx, PreserveEntrySignatures, RuntimeHelper, StmtInfoIdx, SymbolOrMemberExprRef,
  SymbolRef, UsedSymbolRefsBuilder, UsedSymbolRefsView, WrapKind,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;
use crate::esm_init_obligations::collect_entry_reexported_wrapper_inits;

use super::{
  GenerateStage,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  code_splitting::IndexSplittingInfo,
  reduced_chunk_graph::ReducedChunkGraph,
  simulated_facade_inclusion::include_simulated_facade_namespace,
};

struct ChunkAtom {
  modules: Vec<ModuleIdx>,
  dependent_entries: BitSet,
}

/// Atom-level edge graphs for the pass's two consumers. `reachability` under-approximates the
/// emitted chunk imports (safe to miss an edge, never to invent one); `cycle` over-approximates
/// them (safe to invent an edge, never to miss one). See `compute_atom_dependencies`. Under strict
/// execution order `cycle` is empty unless a CommonJS module renders raw, since the fold doesn't
/// otherwise keep the chunk graph acyclic there.
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
  pub(super) fn preserve_dynamic_entry_namespaces(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    used_symbol_refs_builder: &mut UsedSymbolRefsBuilder,
  ) {
    // Even an unmerged JSON facade must expose the same default-only namespace
    // as a static namespace import, not its bundler-only named bindings. And an
    // unmerged facade whose star exports reach CommonJS modules must expose the
    // namespace object that `__reExport` fills at runtime, whose names its chunk's
    // export list can't carry, also when it is an explicit input, an emitted chunk or a
    // preserved module, whose public exports stay (`route_public_namespace_extractions`).
    let namespaces = chunk_graph
      .entry_module_to_entry_chunk
      .iter()
      .filter_map(|(&module, &chunk)| {
        let ChunkKind::EntryPoint { meta, .. } = chunk_graph.chunk_table[chunk].kind else {
          return None;
        };
        let json = meta == ChunkMeta::DynamicImported
          && !self.options.preserve_modules
          && self.link_output.module_table[module]
            .as_normal()
            .is_some_and(crate::utils::import_options::is_json_module);
        (json
          || (meta.contains(ChunkMeta::DynamicImported)
            && self.dynamic_exports_need_namespace_extraction(module)))
        .then_some((chunk, module))
      })
      .collect();
    self.apply_dynamic_entry_namespace_extractions(
      chunk_graph,
      &namespaces,
      used_symbol_refs_builder,
    );
  }

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
    let flattened_entry_modules = self.flattened_entry_modules();
    // Strict execution order makes a chunk cycle safe, so the fold keeps the chunk graph acyclic
    // only without it, or when a module's body would still run raw. There, lowering defers each
    // module that a cycle's evaluation could reach too early: wrap-all defers every eligible
    // module, and on-demand wrapping wraps every eligible module under a root that reaches a chunk
    // cycle and in each chunk that lowering puts on one (`analyze_execution_order`). The modules
    // that aren't eligible are wrapped, and so lazy already, or the runtime, whose standalone chunk
    // imports nothing, or a CommonJS entry that renders raw, which keeps the checks. An entry whose
    // chunk something else loads gets a facade for its trigger (`create_order_wrap_entry_facades`).
    // The strict cycle graph links every import record, so it has a cycle wherever the modules do,
    // and with it the fold never ran for such an application.
    let cycle_merging = crate::utils::topology_config::get(self.options)
      .and_then(|cfg| cfg.cycle_merging)
      .unwrap_or_else(|| {
        if self.options.experimental.is_on_demand_wrapping_enabled() {
          crate::utils::topology_config::CycleMergingPolicy::Forbid
        } else {
          crate::utils::topology_config::CycleMergingPolicy::AllowWrapped
        }
      });
    let keep_acyclic = !self.options.is_strict_execution_order_enabled()
      || self.has_raw_commonjs_module()
      || self.options.experimental.is_on_demand_wrapping_enabled()
      || matches!(cycle_merging, crate::utils::topology_config::CycleMergingPolicy::Forbid);
    let atom_dependencies = self.compute_atom_dependencies(
      &atoms,
      &module_to_atom_idx,
      &flattened_entry_modules,
      used_symbol_refs_builder,
      keep_acyclic,
    );
    // Indexed by entry bit, so `ReducedChunkGraph` can pair each facade chunk with its entry
    // module's host atom.
    let host_atom_by_entry_bit: Vec<Option<usize>> =
      flattened_entry_modules.iter().map(|&module_idx| module_to_atom_idx[module_idx]).collect();

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

    let mut reduced_chunk_graph = ReducedChunkGraph::new(
      atoms.iter().map(|atom| &atom.dependent_entries),
      &atom_dependencies.cycle,
      &host_atom_by_entry_bit,
    );
    let mut changed = false;
    for (atom_idx, atom) in atoms.iter_mut().enumerate() {
      let original_dependent_entries = atom.dependent_entries.clone();
      let dependent_entries = atom.dependent_entries.index_of_one().collect::<Vec<_>>();
      let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
      for entry_idx in dependent_entries {
        if already_loaded_atoms_by_entry[entry_idx as usize].has_bit(atom_bit) {
          atom.dependent_entries.clear_bit(entry_idx);
        }
      }
      let action = if atom.dependent_entries == original_dependent_entries {
        ReducedEntriesAction::Avoid
      } else {
        self.can_use_reduced_dependent_entries(
          atom,
          &original_dependent_entries,
          &atom.dependent_entries,
          chunk_graph,
          &dynamic_entry_modules_by_entry,
        )
      };
      if !matches!(action, ReducedEntriesAction::Avoid)
        && (!keep_acyclic || reduced_chunk_graph.try_move(atom_idx, &atom.dependent_entries))
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
        atom.dependent_entries = original_dependent_entries;
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
    if matches!(
      chunk.kind,
      ChunkKind::EntryPoint { meta, .. } if meta.contains(ChunkMeta::UserDefinedEntry)
    ) && crate::utils::topology_config::get(self.options)
      .is_some_and(|cfg| cfg.preserve_container_entries)
    {
      return ReducedEntriesAction::Avoid;
    }

    // See internal-docs/inline-common-chunks/design.md#keeping-shared-code-available-for-inlining.
    if self.prefers_inline_common_chunk(&atom.modules) {
      return ReducedEntriesAction::Avoid;
    }

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

  /// Whether `import()` of this ES module dynamic entry has to read its namespace object,
  /// because a star export of a CommonJS module, directly or through other modules, adds
  /// names only at runtime, through `__reExport`. Its chunk's export list then lacks them.
  ///
  /// `import()` then assimilates the namespace object, so a `then` among its exports sees
  /// every export through `this`, as it would the module's. The chunk leaves a `then` of the
  /// module's own out of its exports, which would make the chunk a thenable that `import()`
  /// assimilates before the namespace can be read (`hides_entry_then` in
  /// compute_cross_chunk_links.rs), unless it is public and keeps the `then`, when `import()`
  /// reads the namespace from the chunk hosting the module (`route_public_namespace_extractions`).
  /// A star export that reaches an external module is re-exported by the file itself
  /// (`EntryLevelExternal`) instead of the namespace object, which keeps the plain `import()`.
  fn dynamic_exports_need_namespace_extraction(&self, entry_module_idx: ModuleIdx) -> bool {
    let Some(module) = self.link_output.module_table[entry_module_idx].as_normal() else {
      return false;
    };
    let meta = &self.link_output.metas[entry_module_idx];
    if !matches!(module.exports_kind, ExportsKind::Esm) || !meta.has_dynamic_exports {
      return false;
    }
    let mut queue = VecDeque::from([entry_module_idx]);
    let mut visited = FxHashSet::default();
    while let Some(module_idx) = queue.pop_front() {
      if !visited.insert(module_idx) {
        continue;
      }
      let Module::Normal(module) = &self.link_output.module_table[module_idx] else {
        return false;
      };
      for record in &module.import_records {
        if record.meta.contains(ImportRecordMeta::IsExportStar)
          && let Some(importee_idx) = record.resolved_module
        {
          queue.push_back(importee_idx);
        }
      }
    }
    true
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

  /// Routes `import()` of a module that reads its namespace object from a public chunk, an
  /// explicit input's, an emitted chunk's or a preserved module's, to a chunk that it can import
  /// without changing the public chunk's exports, which stay exactly the declared ones, a `then`
  /// among them. Another chunk's `import()` of a module that its public chunk hosts splits the
  /// chunk: the module's code stays in it as a common chunk, whose exports, the namespace object's
  /// among them, take internal names, which are never `then`, and a new public facade re-exports
  /// the module's exports from it. Another chunk's `import()` reads the namespace from the common
  /// chunk hosting the module, or through a facade of its own when that chunk exports a `then`,
  /// which `import()` of it would call. An `import()` inside the hosting chunk reads the namespace
  /// in place.
  pub(super) fn route_public_namespace_extractions(&self, chunk_graph: &mut ChunkGraph) {
    if self.options.code_splitting.is_disabled() {
      return;
    }
    let mut public: Vec<(ChunkIdx, ModuleIdx, u32)> = chunk_graph
      .common_chunk_exported_facade_chunk_namespace
      .iter()
      .filter_map(|(&chunk_idx, modules)| {
        let ChunkKind::EntryPoint { meta, bit, module } = chunk_graph.chunk_table[chunk_idx].kind
        else {
          return None;
        };
        (modules.contains(&module)
          && chunk_graph.chunk_is_live(chunk_idx)
          && (self.options.preserve_modules
            || meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk)))
        .then_some((chunk_idx, module, bit))
      })
      .collect();
    if public.is_empty() {
      return;
    }
    public.sort_unstable();
    let importers =
      self.live_dynamic_importers(chunk_graph, public.iter().map(|&(_, module, _)| module));
    let mut added = false;
    for (chunk_idx, module_idx, bit) in public {
      let Some(host_idx) = chunk_graph.module_to_chunk[module_idx] else {
        continue;
      };
      let crosses = importers[&module_idx]
        .iter()
        .any(|importer| importer.trigger_chunk.is_some_and(|chunk| chunk != host_idx));
      if host_idx == chunk_idx {
        if !crosses {
          continue;
        }
        self.split_public_entry_chunk(chunk_graph, chunk_idx, module_idx);
        added = true;
      } else if !matches!(chunk_graph.chunk_table[host_idx].kind, ChunkKind::Common) {
        // Strict execution order or a manual group leaves the public chunk a facade of a common
        // chunk hosting the module. Another entry's chunk can't take the namespace's exports
        // either, so `import()` resolves with the public chunk's own namespace.
        remove_namespace_extraction(chunk_graph, chunk_idx, module_idx);
        continue;
      }
      let reader_idx = if crosses && self.common_chunk_exports_then(chunk_graph, host_idx) {
        added = true;
        self.add_namespace_extraction_facade(chunk_graph, chunk_idx, module_idx, bit)
      } else {
        host_idx
      };
      remove_namespace_extraction(chunk_graph, chunk_idx, module_idx);
      chunk_graph
        .common_chunk_exported_facade_chunk_namespace
        .entry(reader_idx)
        .or_default()
        .insert(module_idx);
      chunk_graph.entry_module_to_entry_chunk.insert(module_idx, reader_idx);
    }
    if added {
      chunk_graph.rebuild_sorted_chunk_idx_vec(true);
    }
  }

  /// Whether the common chunk `chunk_idx` exports a `then`, which `import()` of it would call
  /// instead of resolving with its namespace. Only a module whose export names the chunk keeps, an
  /// emitted chunk's that allows extensions, can give it one: no other export takes the name (see
  /// `dynamic_entry_supports_namespace_extraction`).
  fn common_chunk_exports_then(&self, chunk_graph: &ChunkGraph, chunk_idx: ChunkIdx) -> bool {
    chunk_graph.common_chunk_preserve_export_names_modules.get(&chunk_idx).is_some_and(|modules| {
      modules.iter().any(|&module_idx| {
        self.link_output.metas[module_idx]
          .resolved_exports
          .keys()
          .any(|name| name.as_str() == "then")
      })
    })
  }

  /// Adds a dynamic entry's facade of `module_idx`, through which `import()` reads the module's
  /// namespace object from the common chunk hosting it without the `then` that this chunk exports.
  /// The facade leaves out the module's own `then` too (`hides_entry_then`). It takes the bits,
  /// input base and execution order of `chunk_idx`, the module's public chunk or the common chunk
  /// that the public chunk became.
  fn add_namespace_extraction_facade(
    &self,
    chunk_graph: &mut ChunkGraph,
    chunk_idx: ChunkIdx,
    module_idx: ModuleIdx,
    bit: u32,
  ) -> ChunkIdx {
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    let mut facade = rolldown_common::Chunk::new(
      None,
      None,
      chunk.bits.clone(),
      vec![],
      ChunkKind::EntryPoint { meta: ChunkMeta::DynamicImported, bit, module: module_idx },
      chunk.input_base.clone(),
      None,
    );
    facade.exec_order = chunk.exec_order;
    facade.add_creation_reason(
      ChunkCreationReason::Entry {
        is_user_defined_entry: false,
        entry_module_id: self.link_output.module_table[module_idx].stable_id(),
        name: None,
      },
      self.options,
    );
    chunk_graph.add_chunk(facade)
  }

  /// Turns the public entry chunk `chunk_idx`, which hosts `module_idx`, into a common chunk behind
  /// a new public facade that takes over its name, file name, signature and emitted references.
  fn split_public_entry_chunk(
    &self,
    chunk_graph: &mut ChunkGraph,
    chunk_idx: ChunkIdx,
    module_idx: ModuleIdx,
  ) {
    let entry_chunk = &mut chunk_graph.chunk_table[chunk_idx];
    let ChunkKind::EntryPoint { meta, bit, .. } = entry_chunk.kind else {
      return;
    };
    let bits = entry_chunk.bits.clone();
    let input_base = entry_chunk.input_base.clone();
    let name = entry_chunk.name.take();
    let file_name = entry_chunk.file_name.take();
    let preserve_entry_signature = entry_chunk.preserve_entry_signature.take();
    let exec_order = entry_chunk.exec_order;
    entry_chunk.kind = ChunkKind::Common;
    entry_chunk.add_creation_reason(
      ChunkCreationReason::CommonChunk { bits: &bits, link_output: self.link_output },
      self.options,
    );
    let mut facade = rolldown_common::Chunk::new(
      name,
      file_name,
      bits,
      vec![],
      ChunkKind::EntryPoint { meta, bit, module: module_idx },
      input_base,
      preserve_entry_signature,
    );
    facade.exec_order = exec_order;
    facade.add_creation_reason(
      ChunkCreationReason::Entry {
        is_user_defined_entry: meta.contains(ChunkMeta::UserDefinedEntry),
        entry_module_id: self.link_output.module_table[module_idx].stable_id(),
        name: self
          .link_output
          .entries
          .get(&module_idx)
          .and_then(|entries| entries.first())
          .and_then(|entry| entry.name.as_ref()),
      },
      self.options,
    );
    let facade_idx = chunk_graph.add_chunk(facade);
    if let Some(reference_ids) = chunk_graph.chunk_idx_to_reference_ids.remove(&chunk_idx) {
      chunk_graph.chunk_idx_to_reference_ids.insert(facade_idx, reference_ids);
    }
  }

  fn is_runtime_only_atom(&self, atom: &ChunkAtom) -> bool {
    atom.modules.len() == 1 && atom.modules[0] == self.link_output.runtime.id()
  }

  /// Whether an included CommonJS module renders raw, without a `require_*` wrapper, as a CommonJS
  /// entry that nothing imports does in cjs output without code-splitting groups (`wrap_modules`).
  /// Its body runs at the top level of its chunk, outside the order analysis, so a chunk cycle
  /// through that chunk can run it before the chunk defines what the cycle imports from it.
  fn has_raw_commonjs_module(&self) -> bool {
    self.link_output.module_table.modules.iter().filter_map(Module::as_normal).any(|module| {
      let meta = &self.link_output.metas[module.idx];
      matches!(module.exports_kind, ExportsKind::CommonJs)
        && meta.is_included
        && matches!(meta.wrap_kind(), WrapKind::None)
    })
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

  /// The entry module behind each entry bit, in the bit order `determine_reachable_modules_for_entry`
  /// assigns (`link_output.entries` flattened, one bit per entry point).
  fn flattened_entry_modules(&self) -> Vec<ModuleIdx> {
    self
      .link_output
      .entries
      .iter()
      .flat_map(|(&idx, entries)| std::iter::repeat_n(idx, entries.len()))
      .collect()
  }

  // See internal-docs/code-splitting/implementation.md#dynamic-already-loaded-analysis.
  //
  // The two graphs serve the pass's two consumers, whose safe approximations point in opposite
  // directions. `reachability` must only contain edges the emitted chunks really have: its
  // entry-export service edges are liveness-gated and attached only when the entry module is
  // hosted by its own entry chunk, because emission hangs those imports on the entry (facade)
  // chunk, not on whichever shared chunk hosts the module. `cycle` must contain every edge the
  // emitted chunks might have: its service edges are attached to the hosting atom regardless,
  // with no liveness gate — `used_symbol_refs_builder` still grows after this pass
  // (namespace-extraction and facade-elimination replays), so an export dead here can be live at
  // emission.
  //
  // Hosting-atom attribution over-approximates rather than misses only because of the facade edges
  // `ReducedChunkGraph` adds: the facade depends on the host, which is where
  // these edges land, so anything emission hangs on the facade stays reachable as facade -> host ->
  // target. Before rolldown#10734 this rested on facades having zero static in-degree, which a
  // fold moving an atom into a facade broke — re-derive it before attributing them elsewhere.
  //
  // The price of the ungated edges is that a dead entry re-export can conservatively veto a fold
  // that would have been acyclic — a missed optimization, accepted over trusting a liveness
  // snapshot the replays may outgrow. The liveness-growth immunity is scoped to service edges:
  // the base prediction's ambiguous branch (`referenced_symbol_owners`) still reads decision-time
  // statement inclusion, a narrower pre-existing skew the replays can also outgrow.
  fn compute_atom_dependencies(
    &self,
    atoms: &[ChunkAtom],
    module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
    flattened_entry_modules: &[ModuleIdx],
    used_symbol_refs_builder: &UsedSymbolRefsBuilder,
    keep_acyclic: bool,
  ) -> AtomDependencyGraphs {
    let strict_execution_order = self.options.is_strict_execution_order_enabled();

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
          // Strict lowering can turn linked import records back into `init_*` imports. Unless the
          // fold keeps the chunk graph acyclic, only reachability needs them.
          for &dep_module_idx in &self.link_output.metas[module_idx].dependencies {
            add(dep_module_idx, &mut reachability_deps);
          }
          if keep_acyclic {
            if self.has_raw_commonjs_module() {
              for &dep_module_idx in &self.link_output.metas[module_idx].dependencies {
                add(dep_module_idx, &mut cycle_deps);
              }
            } else {
              for dep_module_idx in self.predicted_static_import_targets(module_idx) {
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
            }
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

  pub(super) fn referenced_symbol_owners(&self, module_idx: ModuleIdx) -> FxHashSet<ModuleIdx> {
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

  pub(super) fn is_included_dynamic_import_record(
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

/// Stops `import()` of `module_idx` from reading the module's namespace object from `chunk_idx`.
fn remove_namespace_extraction(
  chunk_graph: &mut ChunkGraph,
  chunk_idx: ChunkIdx,
  module_idx: ModuleIdx,
) {
  let namespaces = &mut chunk_graph.common_chunk_exported_facade_chunk_namespace;
  if let Some(modules) = namespaces.get_mut(&chunk_idx) {
    modules.remove(&module_idx);
    if modules.is_empty() {
      namespaces.remove(&chunk_idx);
    }
  }
}
