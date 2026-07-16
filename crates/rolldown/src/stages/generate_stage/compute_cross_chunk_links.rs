use super::GenerateStage;
use crate::chunk_graph::ChunkGraph;
use crate::esm_init_obligations::{
  ObligationPurpose, WrappedEsmInitTargetContext,
  collect_wrapped_esm_init_targets_for_import_record, for_each_init_obligation_record,
};
use crate::utils::chunk::conflict_resolver::{ConflictResolver, deconflict_order_key};
use crate::utils::chunk::normalize_preserve_entry_signature;
use crate::utils::external_import_interop::external_import_needs_interop;
use itertools::{Itertools, multizip};
use oxc_index::{IndexVec, index_vec};
use oxc_str::CompactStr;
use rolldown_common::{
  ChunkIdx, ChunkKind, ChunkMeta, CrossChunkImportItem, EntryPointKind, ExportsKind, ImportKind,
  ImportRecordMeta, Module, ModuleIdx, NamedImport, OutputFormat, PostChunkOptimizationOperation,
  PreserveEntrySignatures, RUNTIME_HELPER_NAMES, RuntimeHelper, SymbolRef, UsedSymbolRefs,
  UsedSymbolRefsBuilder, WrapKind,
};
use rolldown_utils::index_vec_ext::IndexVecRefExt as _;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

type IndexChunkDependedSymbols = IndexVec<ChunkIdx, FxIndexSet<SymbolRef>>;
type IndexChunkImportsFromExternalModules =
  IndexVec<ChunkIdx, FxHashMap<ModuleIdx, Vec<(ModuleIdx, NamedImport)>>>;
type IndexChunkAllImportsFromExternalModules = IndexVec<ChunkIdx, FxIndexSet<ModuleIdx>>;
type IndexChunkExportedSymbols = IndexVec<ChunkIdx, FxHashMap<SymbolRef, Vec<CompactStr>>>;
type IndexCrossChunkImports = IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>;
type IndexCrossChunkDynamicImports = IndexVec<ChunkIdx, FxIndexSet<ChunkIdx>>;
type IndexImportsFromOtherChunks =
  IndexVec<ChunkIdx, FxHashMap<ChunkIdx, Vec<CrossChunkImportItem>>>;

struct CrossChunkLinkState {
  index_chunk_exported_symbols: IndexChunkExportedSymbols,
  index_chunk_direct_imports_from_external_modules: IndexChunkImportsFromExternalModules,
  index_chunk_indirect_imports_from_external_modules: IndexChunkAllImportsFromExternalModules,
  index_imports_from_other_chunks: IndexImportsFromOtherChunks,
  index_cross_chunk_imports: IndexCrossChunkImports,
  index_cross_chunk_dynamic_imports: IndexCrossChunkDynamicImports,
  order_live_symbols: FxHashSet<SymbolRef>,
}

trait UsedSymbolRefsView: Sync {
  fn contains(&self, symbol_ref: &SymbolRef) -> bool;
}

impl UsedSymbolRefsView for UsedSymbolRefs {
  fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    UsedSymbolRefs::contains(self, symbol_ref)
  }
}

impl UsedSymbolRefsView for UsedSymbolRefsBuilder {
  fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    UsedSymbolRefsBuilder::contains(self, symbol_ref)
  }
}

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn compute_cross_chunk_links(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
  ) {
    let CrossChunkLinkState {
      index_chunk_exported_symbols,
      index_chunk_direct_imports_from_external_modules,
      mut index_chunk_indirect_imports_from_external_modules,
      index_imports_from_other_chunks,
      index_cross_chunk_imports,
      index_cross_chunk_dynamic_imports,
      order_live_symbols,
    } = self.compute_cross_chunk_link_state(chunk_graph, used_symbol_refs, order_state);

    #[cfg(debug_assertions)]
    let predicted_static_import_edges: IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> =
      index_imports_from_other_chunks
        .iter_enumerated()
        .map(|(chunk_idx, importee_map)| {
          importee_map
            .keys()
            .copied()
            .chain(chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks.keys().copied())
            .collect()
        })
        .collect();

    self.deconflict_exported_names(
      chunk_graph,
      &index_chunk_exported_symbols,
      used_symbol_refs,
      &order_live_symbols,
    );

    let index_sorted_cross_chunk_imports = index_cross_chunk_imports
      .par_iter_enumerated()
      .map(|(chunk_idx, cross_chunk_imports)| {
        // Include imports from `imports_from_other_chunks` which may have been
        // added during chunk merging optimization (PR #7194).
        // See: https://github.com/rolldown/rolldown/issues/7297
        let mut cross_chunk_imports = cross_chunk_imports
          .iter()
          .copied()
          .chain(chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks.keys().copied())
          .collect::<Vec<_>>();
        cross_chunk_imports
          .sort_unstable_by_key(|chunk_id| chunk_graph.chunk_table[*chunk_id].exec_order);
        cross_chunk_imports
      })
      .collect::<Vec<_>>();

    let index_sorted_imports_from_other_chunks = index_imports_from_other_chunks
      .into_iter_enumerated()
      .map(|(chunk_idx, mut importee_map)| {
        for (idx, items) in &chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks {
          importee_map.entry(*idx).or_default().extend_from_slice(items);
        }
        importee_map
          .into_iter()
          .sorted_unstable_by_key(|(importee_chunk_id, _)| {
            chunk_graph.chunk_table[*importee_chunk_id].exec_order
          })
          .collect::<FxIndexMap<_, _>>()
      })
      .collect::<Vec<_>>();

    let index_sorted_imports_from_external_modules =
      index_chunk_direct_imports_from_external_modules
        .into_iter()
        .map(|imports_from_external_modules| {
          imports_from_external_modules
            .into_iter()
            .sorted_unstable_by_key(|(external_module_id, _)| {
              self.link_output.module_table[*external_module_id].exec_order()
            })
            .collect_vec()
        })
        .collect::<Vec<_>>();

    multizip((
      chunk_graph.chunk_table.iter_mut(),
      index_sorted_imports_from_other_chunks,
      index_sorted_imports_from_external_modules,
      index_sorted_cross_chunk_imports,
      index_cross_chunk_dynamic_imports,
      index_chunk_indirect_imports_from_external_modules.iter_mut(),
    ))
    .par_bridge()
    .for_each(
      |(
        chunk,
        sorted_imports_from_other_chunks,
        imports_from_external_modules,
        cross_chunk_imports,
        cross_chunk_dynamic_imports,
        chunk_indirect_imports_from_external_modules,
      )| {
        // deduplicated
        for (module_idx, _) in &imports_from_external_modules {
          chunk_indirect_imports_from_external_modules.shift_remove(module_idx);
        }
        chunk.imports_from_other_chunks = sorted_imports_from_other_chunks;
        chunk.direct_imports_from_external_modules = imports_from_external_modules;
        chunk.cross_chunk_imports = cross_chunk_imports;
        chunk.cross_chunk_dynamic_imports =
          cross_chunk_dynamic_imports.into_iter().collect::<Vec<_>>();
        chunk.import_symbol_from_external_modules =
          std::mem::take(chunk_indirect_imports_from_external_modules);
      },
    );

    #[cfg(debug_assertions)]
    for (chunk_idx, predicted_edges) in predicted_static_import_edges.into_iter_enumerated() {
      let actual_edges =
        chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks.keys().copied().collect();
      debug_assert_eq!(
        predicted_edges, actual_edges,
        "predicted static chunk import edges diverged for chunk {chunk_idx:?}",
      );
    }

    // Empty entry facades (order-wrap trigger facades and dynamic-entry facades) hold zero modules,
    // so they export no symbols and nothing can depend on them across a *static* import — their only
    // inbound edges are dynamic, routed through `entry_module_to_entry_chunk` outside the static SCC
    // graph. The emergent-cycle projector relies on this to soundly omit facade edges from its
    // static chunk-SCC search (`post_lowering_import_edges` doc): a facade can never sit inside a
    // static cycle, so the "entry-facade transitive init imports" edge source is not constructible.
    // Assert it so a future change that gives a facade static indegree trips here instead of silently
    // defeating the projection.
    #[cfg(debug_assertions)]
    if self.options.is_strict_execution_order_enabled() {
      let empty_facades = chunk_graph
        .chunk_table
        .iter_enumerated()
        .filter(|(_, chunk)| {
          matches!(chunk.kind, ChunkKind::EntryPoint { .. }) && chunk.modules.is_empty()
        })
        .map(|(idx, _)| idx)
        .collect::<FxHashSet<_>>();
      if !empty_facades.is_empty() {
        for chunk in chunk_graph.chunk_table.iter() {
          for importee in chunk.imports_from_other_chunks.keys() {
            debug_assert!(
              !empty_facades.contains(importee),
              "an empty entry facade gained a static import edge, defeating the projector's \
               zero-static-indegree assumption",
            );
          }
        }
      }
    }

    // Final-topology soundness assert for the emergent-cycle projector. Under strict execution
    // order the projector must order-wrap every module that needs deferral when its chunk sits in a
    // static chunk cycle, so the runtime `init_*` forwarding it emits can never reach an
    // uninitialized wrapper. That soundness rests on the projector mirroring the linker's three
    // registration paths; the review noted one consumption disjunct
    // (`referenced_symbols_by_entry_point_chunk`) with no projection counterpart. A future
    // projection hole would otherwise surface only as a fuzzer-caught `init_* is not a function`
    // runtime crash. Re-run Tarjan over the *final* cross-chunk import graph and assert the exact
    // obligation `close_cyclic_chunk_members` discharges — every module hosted in a nontrivial SCC
    // that is both order-sensitive AND order-wrap-eligible is in the plan — turning any such hole
    // into a deterministic debug/CI build failure. The obligation is deliberately not raw
    // `is_order_wrap_eligible`: a side-effect-free, import-free eligible leaf (e.g.
    // `export const x = 1`) is never wrapped by the projector, yet can be co-hosted in a chunk that
    // lands in a cycle because of its *siblings'* cross-chunk imports, and leaving it unwrapped is
    // correct. This is a structural invariant like the facade assert above, not the internal
    // semantic verifier `design.md` rejects.
    #[cfg(debug_assertions)]
    if self.options.is_strict_execution_order_enabled() {
      let mut graph = petgraph::prelude::DiGraphMap::<ChunkIdx, ()>::new();
      for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
        graph.add_node(chunk_idx);
        for &importee_idx in chunk.imports_from_other_chunks.keys() {
          graph.add_edge(chunk_idx, importee_idx, ());
        }
      }
      for scc in petgraph::algo::tarjan_scc(&graph) {
        // Nontrivial = a real cycle: two-plus chunks, or a single chunk with a self-edge. Trivial
        // singletons cannot host an init cycle, so they carry no wrapping obligation.
        let is_nontrivial = scc.len() >= 2
          || chunk_graph.chunk_table[scc[0]].imports_from_other_chunks.contains_key(&scc[0]);
        if !is_nontrivial {
          continue;
        }
        for &chunk_idx in &scc {
          for &module_idx in &chunk_graph.chunk_table[chunk_idx].modules {
            debug_assert!(
              !(self.is_order_sensitive(module_idx) && self.is_order_wrap_eligible(module_idx))
                || order_state.has_order_wrapper(module_idx),
              "order-sensitive, order-wrap-eligible module {module_idx:?} hosted in chunk \
               {chunk_idx:?} sits in a nontrivial static chunk SCC but was not order-wrapped by the \
               plan — the emergent-cycle projector under-projected this cycle",
            );
          }
        }
      }
    }
  }

  /// Compute provisional links for order analysis. Runtime symbol placement is cleared if moved.
  /// Uses an empty order state, so the edges are the *pre-lowering* baseline topology (value and
  /// side-effect imports, before any wrapping adds `init_*` wrapper imports). The emergent-cycle
  /// fixpoint layers the plan's `init_*` forwarding edges on top of this baseline
  /// (`post_lowering_import_edges`).
  pub(super) fn predicted_static_import_edges(
    &mut self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &UsedSymbolRefsBuilder,
  ) -> IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> {
    let empty_order_state = super::order_wrap_state::OrderWrapState::default();
    self
      .compute_cross_chunk_link_state(chunk_graph, used_symbol_refs, &empty_order_state)
      .index_imports_from_other_chunks
      .into_iter_enumerated()
      .map(|(chunk_idx, importee_map)| {
        importee_map
          .into_keys()
          .chain(chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks.keys().copied())
          .collect()
      })
      .collect()
  }

  fn compute_cross_chunk_link_state(
    &mut self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &impl UsedSymbolRefsView,
    order_state: &super::order_wrap_state::OrderWrapState,
  ) -> CrossChunkLinkState {
    let mut index_chunk_depended_symbols: IndexChunkDependedSymbols =
      index_vec![FxIndexSet::<SymbolRef>::default(); chunk_graph.chunk_table.len()];
    let mut index_chunk_exported_symbols: IndexChunkExportedSymbols =
      index_vec![FxHashMap::<SymbolRef, Vec<CompactStr>>::default(); chunk_graph.chunk_table.len()];
    let mut index_chunk_direct_imports_from_external_modules: IndexChunkImportsFromExternalModules = index_vec![FxHashMap::<ModuleIdx, Vec<(ModuleIdx, NamedImport)>>::default(); chunk_graph.chunk_table.len()];
    // Used for cjs,umd,iife only
    let mut index_chunk_indirect_imports_from_external_modules: IndexChunkAllImportsFromExternalModules =
      index_vec![FxIndexSet::<ModuleIdx>::default(); chunk_graph.chunk_table.len()];

    let mut index_imports_from_other_chunks: IndexImportsFromOtherChunks = index_vec![FxHashMap::<ChunkIdx, Vec<CrossChunkImportItem>>::default(); chunk_graph.chunk_table.len()];
    let mut index_cross_chunk_imports: IndexCrossChunkImports =
      index_vec![FxHashSet::default(); chunk_graph.chunk_table.len()];
    let mut index_cross_chunk_dynamic_imports: IndexCrossChunkDynamicImports =
      index_vec![FxIndexSet::default(); chunk_graph.chunk_table.len()];
    let rendered_modules =
      order_state.has_import_overlays().then(|| super::rendered_module_set(chunk_graph));
    let symbols = &self.link_output.symbol_db;
    let runtime = &self.link_output.runtime;
    let order_live_symbols = order_state.live_symbols(
      |symbol_ref| symbols.canonical_ref_resolving_namespace(symbol_ref),
      |helper| {
        let index = helper.bits().trailing_zeros() as usize;
        runtime.resolve_symbol(RUNTIME_HELPER_NAMES[index])
      },
      |importer_idx| {
        rendered_modules
          .as_ref()
          .is_some_and(|rendered_modules| rendered_modules.contains(&importer_idx))
      },
    );

    self.collect_depended_symbols(
      chunk_graph,
      &mut index_chunk_depended_symbols,
      &mut index_chunk_direct_imports_from_external_modules,
      &mut index_cross_chunk_dynamic_imports,
      used_symbol_refs,
      order_state,
    );

    self.compute_chunk_imports(
      chunk_graph,
      &index_chunk_depended_symbols,
      &index_chunk_direct_imports_from_external_modules,
      &mut index_chunk_exported_symbols,
      &mut index_cross_chunk_imports,
      &mut index_imports_from_other_chunks,
      &mut index_chunk_indirect_imports_from_external_modules,
      used_symbol_refs,
      order_state,
      &order_live_symbols,
    );

    CrossChunkLinkState {
      index_chunk_exported_symbols,
      index_chunk_direct_imports_from_external_modules,
      index_chunk_indirect_imports_from_external_modules,
      index_imports_from_other_chunks,
      index_cross_chunk_imports,
      index_cross_chunk_dynamic_imports,
      order_live_symbols,
    }
  }

  /// - Assign each symbol to the chunk it belongs to
  /// - Collect all referenced symbols and consider them potential imports
  fn collect_depended_symbols(
    &mut self,
    chunk_graph: &ChunkGraph,
    index_chunk_depended_symbols: &mut IndexChunkDependedSymbols,
    index_chunk_imports_from_external_modules: &mut IndexChunkImportsFromExternalModules,
    index_cross_chunk_dynamic_imports: &mut IndexCrossChunkDynamicImports,
    used_symbol_refs: &impl UsedSymbolRefsView,
    order_state: &super::order_wrap_state::OrderWrapState,
  ) {
    let symbols = &self.link_output.symbol_db;
    let chunk_id_to_symbols_vec = append_only_vec::AppendOnlyVec::new();

    let chunks_iter = multizip((
      chunk_graph.chunk_table.iter_enumerated(),
      index_chunk_depended_symbols.iter_mut(),
      index_chunk_imports_from_external_modules.iter_mut(),
      index_cross_chunk_dynamic_imports.iter_mut(),
    ));

    chunks_iter.par_bridge().for_each(
      |(
        (chunk_id, chunk),
        depended_symbols,
        imports_from_external_modules,
        cross_chunk_dynamic_imports,
      )| {
        let mut symbol_needs_to_assign = vec![];
        chunk.modules.iter().copied().for_each(|module_id| {
          let Module::Normal(module) = &self.link_output.module_table[module_id] else {
            return;
          };
          module
            .import_records
            .iter()
            .filter_map(|rec| rec.resolved_module.map(|module_idx| (rec, module_idx)))
            .for_each(|(rec, module_idx)| {
              match &self.link_output.module_table[module_idx] {
                Module::Normal(_) => {
                  // The the resolved module is not included in module graph, skip it.
                  if !self.link_output.metas[module_idx].is_included {
                    return;
                  }
                  if matches!(rec.kind, ImportKind::DynamicImport) {
                    let importee_chunk =
                      chunk_graph.module_to_chunk[module_idx].expect("importee chunk should exist");
                    cross_chunk_dynamic_imports.insert(importee_chunk);
                  }
                }
                Module::External(_) => {
                  // Ensure the external module is imported in case it has side effects.
                  if matches!(rec.kind, ImportKind::Import)
                    && !rec.meta.contains(ImportRecordMeta::IsExportStar)
                  {
                    imports_from_external_modules.entry(module_idx).or_default();
                  }
                }
              }
            });

          module
            .named_imports
            .iter()
            .filter_map(|(_, import)| {
              module.import_records[import.record_idx]
                .resolved_module
                .map(|module_idx| (import, module_idx))
            })
            .for_each(|(import, module_idx)| {
              if let Module::External(importee) = &self.link_output.module_table[module_idx] {
                imports_from_external_modules
                  .entry(importee.idx)
                  .or_default()
                  .push((module.idx, import.clone()));
              }
            });
          self.link_output.stmt_infos[module.idx].iter_enumerated().for_each(
            |(stmt_info_idx, stmt_info)| {
              let is_order_runtime_stmt =
                order_state.forces_runtime_stmt(&self.link_output.runtime, module.idx, stmt_info);
              if !self.link_output.metas[module.idx].stmt_info_included.has_bit(stmt_info_idx)
                && !is_order_runtime_stmt
              {
                return;
              }
              stmt_info.declared_symbols.iter().for_each(|declared| {
                symbol_needs_to_assign.push(*declared);
              });

              stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
                match reference_ref {
                  rolldown_common::SymbolOrMemberExprRef::Symbol(referenced) => {
                    self.add_depended_symbol_with_wrapped_esm_init(
                      chunk_graph,
                      order_state,
                      depended_symbols,
                      symbols.canonical_ref_resolving_namespace(*referenced),
                    );
                  }
                  rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
                    match member_expr.represent_symbol_ref(
                      &self.link_output.metas[module.idx].resolved_member_expr_refs,
                    ) {
                      Some(sym_ref) => {
                        self.add_depended_symbol_with_wrapped_esm_init(
                          chunk_graph,
                          order_state,
                          depended_symbols,
                          symbols.canonical_ref_resolving_namespace(sym_ref),
                        );
                      }
                      _ => {
                        // `None` means the member expression resolve to a ambiguous export, which means it actually resolve to nothing.
                        // It would be rewrite to `undefined` in the final code, so we don't need to include anything to make `undefined` work.
                      }
                    }
                  }
                }
              });
            },
          );
          self.add_module_esm_init_depended_symbols(
            chunk_graph,
            used_symbol_refs,
            order_state,
            depended_symbols,
            module.idx,
          );
        });

        if let Some(entry_id) = &chunk.entry_module_idx() {
          let entry = &self.link_output.module_table[*entry_id].as_normal().unwrap();
          let entry_meta = &self.link_output.metas[entry.idx];

          if !matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
            for export_ref in entry_meta
              .resolved_exports
              .iter()
              .sorted_unstable_by_key(|(name, _)| *name)
              .map(|(_, export)| export)
              // A chunk should always consume a cjs export symbol by property access, so filter
              // out a exported symbol that came from a cjs module.
              .filter(|resolved_export| !resolved_export.came_from_commonjs)
            {
              self.add_depended_symbol_with_wrapped_esm_init(
                chunk_graph,
                order_state,
                depended_symbols,
                symbols.canonical_ref_resolving_namespace(export_ref.symbol_ref),
              );
            }
          }

          if matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
            depended_symbols
              .insert(entry_meta.wrapper_ref.expect("CJS entry should have a wrapper"));
          } else if let Some(target) = order_state.esm_init_target(entry.idx, entry_meta) {
            depended_symbols.insert(target.wrapper_ref);
          }
          // Strict-gated: this feeds order-wrapped entries' prologue init imports. Flag-off, legacy
          // interop `transitive_esm_init_targets` still exist, so for an interop-wrapped entry
          // rendered behind a facade chunk, with an excluded re-export whose wrapped targets share
          // the entry's host chunk, an ungated call would give the facade a dead cross-chunk
          // `init_*` import the pre-#10104 base never emitted. The per-module call in
          // `add_module_esm_init_depended_symbols` stays ungated — it is provably inert flag-off
          // (legacy targets are same-chunk-only, and the cross-chunk filter skips them).
          if self.options.is_strict_execution_order_enabled() {
            self.add_transitive_esm_init_depended_symbols(
              chunk_graph,
              order_state,
              depended_symbols,
              entry.idx,
            );
          }

          if matches!(self.options.format, OutputFormat::Cjs)
            && matches!(entry.exports_kind, ExportsKind::Esm)
          {
            depended_symbols.insert(self.link_output.runtime.resolve_symbol("__toCommonJS"));
            depended_symbols.insert(entry.namespace_object_ref);
          }
        }

        for synthetic in order_state.synthetic_statements_for_chunk(chunk_id) {
          symbol_needs_to_assign.extend(synthetic.declared_symbols.iter().copied());
          for referenced in &synthetic.referenced_symbols {
            self.add_depended_symbol_with_wrapped_esm_init(
              chunk_graph,
              order_state,
              depended_symbols,
              symbols.canonical_ref_resolving_namespace(*referenced),
            );
          }
          for helper in synthetic.runtime_helpers {
            let index = helper.bits().trailing_zeros() as usize;
            depended_symbols
              .insert(self.link_output.runtime.resolve_symbol(RUNTIME_HELPER_NAMES[index]));
          }
        }

        // Depending runtime helpers
        for helper in chunk.depended_runtime_helper {
          let index = helper.bits().trailing_zeros() as usize;
          let name = RUNTIME_HELPER_NAMES[index];
          depended_symbols.insert(self.link_output.runtime.resolve_symbol(name));
        }

        chunk_id_to_symbols_vec.push((chunk_id, symbol_needs_to_assign));
      },
    );
    // shadowing previous immutable borrow
    let symbols = &mut self.link_output.symbol_db;
    for (chunk_idx, symbol_list) in chunk_id_to_symbols_vec {
      for declared in symbol_list {
        let declared = declared.inner();
        if cfg!(debug_assertions) {
          let symbol_data = symbols.get(declared);
          debug_assert!(
            symbol_data.chunk_idx.unwrap_or(chunk_idx) == chunk_idx,
            "Symbol: {:?}, {:?} in {:?} should only belong to one chunk. Existed {:?}, new {chunk_idx:?}",
            declared.name(symbols),
            declared,
            self.link_output.module_table[declared.owner].id().as_str(),
            symbol_data.chunk_idx,
          );
        }

        let symbol_data = symbols.get_mut(declared);
        symbol_data.chunk_idx = Some(chunk_idx);
      }
    }
  }

  fn add_depended_symbol_with_wrapped_esm_init(
    &self,
    chunk_graph: &ChunkGraph,
    order_state: &super::order_wrap_state::OrderWrapState,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    symbol_ref: SymbolRef,
  ) {
    let meta = &self.link_output.metas[symbol_ref.owner];
    if !self.options.is_strict_execution_order_enabled() {
      // Off-strict keeps main's exact shape: lowering never mutates the chunk graph, so the
      // liveness guards below can never fire.
      depended_symbols.insert(symbol_ref);
      if matches!(meta.wrap_kind(), WrapKind::Esm)
        && let Some(wrapper_ref) = meta.wrapper_ref
        && wrapper_ref != symbol_ref
      {
        depended_symbols.insert(wrapper_ref);
      }
      return;
    }

    if matches!(self.link_output.module_table[symbol_ref.owner], Module::Normal(_))
      && !chunk_graph.module_is_in_live_chunk(symbol_ref.owner)
    {
      return;
    }

    if let Some(target) = order_state.esm_init_target(symbol_ref.owner, meta) {
      let target_is_live = order_state.init_target_included_in_live_chunk(
        &target,
        meta,
        symbol_ref.owner,
        chunk_graph,
      );
      if target.wrapper_ref == symbol_ref && !target_is_live {
        return;
      }
      depended_symbols.insert(symbol_ref);
      if target.wrapper_ref != symbol_ref && target_is_live {
        depended_symbols.insert(target.wrapper_ref);
      }
      return;
    }

    depended_symbols.insert(symbol_ref);
  }

  /// All ESM `init_*` wrappers a module's chunk must reach: its excluded re-export forwards
  /// (`transitive_init_targets`), its *included* static-import forwards (a wrapped module evaluates
  /// every module it imports, even cross-chunk), and its order-import overlays.
  fn add_module_esm_init_depended_symbols(
    &self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &impl UsedSymbolRefsView,
    order_state: &super::order_wrap_state::OrderWrapState,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    module_idx: ModuleIdx,
  ) {
    self.add_transitive_esm_init_depended_symbols(
      chunk_graph,
      order_state,
      depended_symbols,
      module_idx,
    );
    self.add_included_import_esm_init_depended_symbols(
      chunk_graph,
      used_symbol_refs,
      order_state,
      depended_symbols,
      module_idx,
    );
    self.add_order_import_overlay_depended_symbols(
      chunk_graph,
      order_state,
      depended_symbols,
      module_idx,
    );
  }

  fn add_transitive_esm_init_depended_symbols(
    &self,
    chunk_graph: &ChunkGraph,
    order_state: &super::order_wrap_state::OrderWrapState,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    module_idx: ModuleIdx,
  ) {
    let meta = &self.link_output.metas[module_idx];
    // Iterate the targets in a deterministic, cross-target-stable order. The map is an
    // `FxHashMap<StmtInfoIdx, _>`, and its `values()` order follows FxHash bucket layout. FxHash is
    // unseeded but hashes differently on 32-bit vs 64-bit, so `values()` visits buckets in a
    // different order on native (64-bit) than on wasm32/WASI. That order flows straight into
    // `depended_symbols` (an `FxIndexSet`), whose insertion order drives the chunk's imported-symbol
    // rename order (the `$1`/`$2` suffixes) in `deconflict_chunk_symbols` — so a hash-ordered walk
    // here makes native and WASI builds resolve rename collisions differently. Sorting by the owning
    // `StmtInfoIdx` pins one order for every target.
    for (_, targets) in order_state
      .transitive_init_targets(module_idx, meta)
      .iter()
      .sorted_unstable_by_key(|(stmt_info_idx, _)| **stmt_info_idx)
    {
      for &target_idx in targets {
        let meta = &self.link_output.metas[target_idx];
        if let Some(target) = order_state.esm_init_target(target_idx, meta)
          && order_state.init_target_included_in_live_chunk(&target, meta, target_idx, chunk_graph)
        {
          depended_symbols.insert(target.wrapper_ref);
        }
      }
    }
  }

  /// A wrapped module's `init_*` forwards to the `init_*` of every module it statically imports
  /// through an *included* import statement (ESM evaluates an imported module when the importer is
  /// evaluated). The finalizer only emits those `init_*()` calls when the target wrapper is
  /// reachable in the importer's chunk, so a cross-chunk target — e.g. a package barrel that
  /// plain-imports and re-exports a side-effect-free component whose value the app consumes directly
  /// from the component's own chunk — must be registered here or its `init_*` would never be
  /// imported, leaving it with zero call sites. This mirrors the finalizer's own target resolution
  /// (`collect_wrapped_esm_init_targets_for_import_record`) so registration and emission stay in
  /// lockstep; a same-chunk or genuinely-eager target is filtered out by
  /// `init_target_included_in_live_chunk`.
  fn add_included_import_esm_init_depended_symbols(
    &self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &impl UsedSymbolRefsView,
    order_state: &super::order_wrap_state::OrderWrapState,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    module_idx: ModuleIdx,
  ) {
    if !self.options.is_strict_execution_order_enabled() {
      return;
    }
    let meta = &self.link_output.metas[module_idx];
    // Only modules that carry their own ESM init wrapper forward inits for their imports.
    if order_state.esm_init_target(module_idx, meta).is_none() {
      return;
    }
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return;
    };
    let Some(chunk_idx) = chunk_graph.module_to_chunk[module_idx] else {
      return;
    };
    let ctx = WrappedEsmInitTargetContext {
      importer: module,
      importer_meta: meta,
      modules: &self.link_output.module_table.modules,
      metas: &self.link_output.metas,
      stmt_infos: &self.link_output.stmt_infos,
      symbol_db: &self.link_output.symbol_db,
      constant_value_map: &self.link_output.global_constant_symbol_map,
      inline_const_mode: self.options.optimization.inline_const.map(|config| config.mode),
      order_wrap_state: order_state,
      strict_execution_order: self.options.is_strict_execution_order_enabled(),
    };
    // Enumerate this importer's obligation records through the shared purpose-gated enumerator
    // (Register contract: included statements, nested records skipped — emission's own gate), then
    // resolve the targets exactly as the finalizer will, but pretend every wrapper is reachable:
    // we are registering precisely so it becomes reachable.
    for_each_init_obligation_record(
      ObligationPurpose::Register,
      module,
      meta,
      &self.link_output.stmt_infos,
      order_state,
      |rec_idx| {
        let targets = collect_wrapped_esm_init_targets_for_import_record(
          &ctx,
          rec_idx,
          |symbol_ref| used_symbol_refs.contains(&symbol_ref),
          |_| true,
          |forwarding_module_idx| {
            chunk_graph.module_to_chunk[forwarding_module_idx] == Some(chunk_idx)
          },
        );
        for target_idx in targets {
          let target_meta = &self.link_output.metas[target_idx];
          if let Some(target) = order_state.esm_init_target(target_idx, target_meta)
            && order_state.init_target_included_in_live_chunk(
              &target,
              target_meta,
              target_idx,
              chunk_graph,
            )
          {
            depended_symbols.insert(target.wrapper_ref);
          }
        }
      },
    );
  }

  fn add_order_import_overlay_depended_symbols(
    &self,
    chunk_graph: &ChunkGraph,
    order_state: &super::order_wrap_state::OrderWrapState,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    importer_idx: ModuleIdx,
  ) {
    for (_, overlay) in order_state.import_overlays_for_importer(importer_idx) {
      debug_assert!(
        !overlay.reexports_dynamic_exports
          || (overlay.runtime_helpers.contains(RuntimeHelper::ReExport)
            && overlay.requires_importer_namespace
            && overlay.requires_importee_namespace)
      );
      for referenced in &overlay.referenced_symbols {
        self.add_depended_symbol_with_wrapped_esm_init(
          chunk_graph,
          order_state,
          depended_symbols,
          self.link_output.symbol_db.canonical_ref_resolving_namespace(*referenced),
        );
      }
      for helper in overlay.runtime_helpers {
        let index = helper.bits().trailing_zeros() as usize;
        depended_symbols
          .insert(self.link_output.runtime.resolve_symbol(RUNTIME_HELPER_NAMES[index]));
      }
    }
  }

  /// - Filter out depended symbols to come from other chunks
  /// - Mark exports of importee chunks
  #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
  fn compute_chunk_imports(
    &self,
    chunk_graph: &ChunkGraph,
    index_chunk_depended_symbols: &IndexChunkDependedSymbols,
    index_chunk_direct_imports_from_external_modules: &IndexChunkImportsFromExternalModules,
    index_chunk_exported_symbols: &mut IndexChunkExportedSymbols,
    index_cross_chunk_imports: &mut IndexCrossChunkImports,
    index_imports_from_other_chunks: &mut IndexImportsFromOtherChunks,
    index_chunk_indirect_imports_from_external_modules: &mut IndexChunkAllImportsFromExternalModules,
    used_symbol_refs: &impl UsedSymbolRefsView,
    order_state: &super::order_wrap_state::OrderWrapState,
    order_live_symbols: &FxHashSet<SymbolRef>,
  ) {
    // For each module that has been absorbed as a facade namespace, we need to know
    // which other modules dynamically import it so we can tell whether the absorbed
    // namespace must be published cross-chunk. `EntryPoint::related_stmt_infos` only
    // covers `DynamicImport`-kind entries; emitted entries that are also dynamically
    // imported (e.g. via `this.emitFile` + `import()` in the same build) wouldn't be
    // found that way. Walking import_records directly catches both.
    let dynamic_importers_by_target: FxHashMap<ModuleIdx, FxHashSet<ModuleIdx>> = {
      let mut map: FxHashMap<ModuleIdx, FxHashSet<ModuleIdx>> = FxHashMap::default();
      let absorbed_targets: FxHashSet<ModuleIdx> = chunk_graph
        .common_chunk_exported_facade_chunk_namespace
        .values()
        .flatten()
        .copied()
        .collect();
      if !absorbed_targets.is_empty() {
        for (importer_idx, module) in self.link_output.module_table.iter_enumerated() {
          let Some(module) = module.as_normal() else { continue };
          for rec in &module.import_records {
            if rec.kind == ImportKind::DynamicImport
              && let Some(resolved) = rec.resolved_module
              && absorbed_targets.contains(&resolved)
            {
              map.entry(resolved).or_default().insert(importer_idx);
            }
          }
        }
      }
      map
    };

    chunk_graph
      .chunk_table
      .iter_enumerated()
      // Skip chunks that are purely removed (merged into other chunks without preserving exports).
      // Chunks with PreserveExports flag (e.g., emitted chunks merged into common chunks) are kept
      // because their exports still need to be computed.
      .filter(|(chunk_id, _)| {
        !chunk_graph
          .post_chunk_optimization_operations
          .get(chunk_id)
          .map(|flag| *flag == PostChunkOptimizationOperation::Removed)
          .unwrap_or(false)
      })
      .for_each(|(chunk_id, chunk)| {
        if let ChunkKind::EntryPoint { module: module_idx, meta, .. } = chunk.kind {
          let is_dynamic_imported = meta.contains(ChunkMeta::DynamicImported);
          let is_user_defined =
            meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk);

          let normalized_entry_signatures = normalize_preserve_entry_signature(
            &self.link_output.overrode_preserve_entry_signature_map,
            self.options,
            module_idx,
          );
          // Under `preserveModules`, every module is emitted as its own file that must mirror its
          // full declared export interface, so always emit the entry signature — the
          // `is_user_defined` / `is_dynamic_imported` / `preserveEntrySignatures` narrowing does not
          // apply (see the "preserve_entry_signatures has no effect" contract in
          // `code_splitting.rs`). The synthetic runtime module is the one exception: it is an
          // internal implementation detail, not a user file imported by path, so its helpers stay
          // demand-driven (exported only when another chunk imports them), exactly as before.
          let is_preserved_user_module =
            self.options.preserve_modules && module_idx != self.link_output.runtime.id();
          let needs_export_entry_signatures = if self.options.preserve_modules {
            is_preserved_user_module || is_dynamic_imported
          } else {
            is_dynamic_imported
              || !matches!(normalized_entry_signatures, PreserveEntrySignatures::False)
          };
          if needs_export_entry_signatures {
            // If the entry point is external, we don't need to compute exports.
            let meta = &self.link_output.metas[module_idx];
            // `preserveModules` emits the complete interface (`UserDefined` kind bypasses the
            // dynamic-import partial-export trimming); otherwise honor the entry's actual kind.
            let entry_point_kind = if is_preserved_user_module || is_user_defined {
              EntryPointKind::UserDefined
            } else {
              EntryPointKind::DynamicImport
            };
            for (name, symbol) in meta
              .referenced_canonical_exports_symbols(
                module_idx,
                entry_point_kind,
                &self.link_output.dynamic_import_exports_usage_map,
                false,
              )
              .map(|(name, export)| (name, export.symbol_ref))
            {
              // `preserveModules` emits a module's complete declared interface (#9934). A JSON
              // module synthesizes a named export per top-level key, but the finalizer
              // (`try_inline_json_module_prop`) may fold a key's `var` binding into the
              // self-contained default-export object, leaving no standalone declaration. Listing
              // such a key here produces an `export { key }` with no binding ->
              // `SyntaxError: Export 'x' is not defined in module` (#10020).
              //
              // Drop a key iff the finalizer inlines it away. That decision is gated on
              // `need_inline_json_prop` (see `finalizer_context.rs`): JSON, ESM exports, and the
              // module namespace object NOT included; and within that, a key is inlined iff it is
              // absent from `json_module_none_self_reference_included_symbol` (i.e. not reached by
              // a named import, entry export, or — keeping every key materialized — a namespace
              // import). Mirror that full condition so the export interface never lists an
              // inlined-away key, while a namespace-imported JSON chunk still exports its complete
              // interface (every key keeps its binding).
              if let Module::Normal(normal_module) = &self.link_output.module_table[module_idx]
                && let Some(none_self_referenced) =
                  normal_module.json_module_none_self_reference_included_symbol.as_deref()
                && !normal_module.exports_kind.is_commonjs()
                && !self.link_output.metas[module_idx].namespace_included
                && !none_self_referenced
                  .contains(&self.link_output.symbol_db.canonical_ref_for(symbol))
              {
                continue;
              }
              index_chunk_exported_symbols[chunk_id].entry(symbol).or_default().push(name.clone());
            }
          }
        }

        // A chunk that absorbed a dynamic-entry facade must publish that absorbed
        // entry's namespace/wrapper so the importer's rewritten dynamic import can
        // extract it via `.then(n => n.<ns>)`. Applies regardless of chunk kind:
        // a `DynamicEntryMergedIntoUserDefinedEntry` elimination puts the entry
        // into a `ChunkKind::EntryPoint`, while a `DynamicEntryMergedIntoCommonChunk`
        // elimination puts it into a `ChunkKind::Common`.
        //
        // We only publish the export when at least one dynamic importer lives in
        // a different chunk. Same-chunk dynamic imports take the
        // `Promise.resolve().then(() => (init_xxx(), namespace))` path in
        // `rewrite_dynamic_import_for_merged_entry` and never read from the
        // surrounding chunk's exports, so the export would otherwise be dead.
        if let Some(set) = chunk_graph.common_chunk_exported_facade_chunk_namespace.get(&chunk_id) {
          for dynamic_entry_module in set {
            let has_external_dynamic_importer =
              dynamic_importers_by_target.get(dynamic_entry_module).is_some_and(|importers| {
                importers.iter().any(|importer_idx| {
                  chunk_graph.module_to_chunk[*importer_idx]
                    .is_some_and(|importer_chunk_idx| importer_chunk_idx != chunk_id)
                })
              });
            if !has_external_dynamic_importer {
              continue;
            }
            let meta = &self.link_output.metas[*dynamic_entry_module];
            if matches!(meta.wrap_kind(), WrapKind::Cjs) {
              // For CJS modules, export only wrapper_ref (require_xxx)
              // Generated code: `import('./chunk.js').then((n) => __toESM(n.require_xxx()))`
              if let Some(wrapper_ref) = meta.wrapper_ref {
                index_chunk_exported_symbols[chunk_id].entry(wrapper_ref).or_default();
              }
            } else if let Some(target) = order_state.esm_init_target(*dynamic_entry_module, meta) {
              // For ESM modules, export both wrapper_ref (init_xxx) and namespace
              // Generated code: `import('./chunk.js').then((n) => (n.init_xxx(), n.namespace))`
              index_chunk_exported_symbols[chunk_id].entry(target.wrapper_ref).or_default();
              let ns_ref = self.link_output.module_table[*dynamic_entry_module]
                .namespace_object_ref()
                .expect("dynamic entry should be normal module");
              index_chunk_exported_symbols[chunk_id].entry(ns_ref).or_default();
            } else {
              // For non-wrapped modules, export only namespace
              // Generated code: `import('./chunk.js').then((n) => n.namespace)`
              let ns_ref = self.link_output.module_table[*dynamic_entry_module]
                .namespace_object_ref()
                .expect("dynamic entry should be normal module");
              index_chunk_exported_symbols[chunk_id].entry(ns_ref).or_default();
            }
          }
        }

        let chunk_meta_imports = &index_chunk_depended_symbols[chunk_id];
        for import_ref in chunk_meta_imports.iter().copied() {
          // Depended symbols are over-collected; drop refs that are not live. A normal
          // module's namespace ref answers to the namespace decision; everything else to
          // the inclusion fixpoint (whose dead refs here are constants that got inlined —
          // constants kept as bindings, e.g. entry exports, stay live — and over-collected
          // refs).
          let is_live = if let Some(m) = self.link_output.module_table[import_ref.owner].as_normal()
            && m.namespace_object_ref == import_ref
          {
            self.link_output.metas[import_ref.owner].namespace_included
          } else {
            non_namespace_symbol_is_live(used_symbol_refs, order_live_symbols, import_ref)
          };
          if !is_live {
            continue;
          }
          // If the symbol from external module and the format is commonjs, we might need to insert runtime
          // symbol ref `__toESM` if it's being used (for namespace or default imports)
          // related to https://github.com/rolldown/rolldown/blob/c100a53c6cfc67b4f92e230da072eef8494862ef/crates/rolldown/src/ecmascript/format/cjs.rs?plain=1#L120-L124
          let import_ref = if self.link_output.module_table[import_ref.owner].is_external() {
            index_chunk_indirect_imports_from_external_modules[chunk_id].insert(import_ref.owner);
            if matches!(self.options.format, OutputFormat::Esm) {
              continue;
            }

            if !index_chunk_direct_imports_from_external_modules[chunk_id]
              .get(&import_ref.owner)
              .is_some_and(|imports| external_import_needs_interop(imports))
            {
              continue;
            }

            // Note: `__toESM` might have been referenced during `collect_depended_symbols` for
            // namespace or default imports from external modules. Named-only imports render as
            // direct `require()` bindings and must not inherit another chunk's `__toESM`.
            let to_esm_ref = self.link_output.runtime.resolve_symbol("__toESM");
            if self.link_output.symbol_db.get(to_esm_ref).chunk_idx.is_some() {
              // __toESM is in a chunk, so it's being used
              to_esm_ref
            } else {
              // __toESM is not being used, so skip this import
              // This happens when the interop helper was optimized away.
              continue;
            }
          } else {
            import_ref
          };
          let import_symbol = self.link_output.symbol_db.get(import_ref);
          let importee_chunk_idx = import_symbol.chunk_idx.unwrap_or_else(|| {
            let symbol_owner = &self.link_output.module_table[import_ref.owner];
            let symbol_name = import_ref.name(&self.link_output.symbol_db);
            panic!(
              "Symbol {:?} in {:?} should belong to a chunk",
              symbol_name,
              symbol_owner.id().as_str()
            )
          });
          // Check if the import is from another chunk
          if chunk_id != importee_chunk_idx {
            index_cross_chunk_imports[chunk_id].insert(importee_chunk_idx);
            let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
            imports_from_other_chunks
              .entry(importee_chunk_idx)
              .or_default()
              .push(CrossChunkImportItem { import_ref });
            index_chunk_exported_symbols[importee_chunk_idx].entry(import_ref).or_default();
          }
        }

        if let ChunkKind::EntryPoint { module: entry_module_idx, .. } = &chunk.kind {
          // If the entry module is in a different chunk (facade entry), ensure that chunk
          // is imported. Without this, the facade would be empty and the entry module's
          // code would never execute.
          if let Some(entry_chunk_idx) = chunk_graph.module_to_chunk[*entry_module_idx] {
            if entry_chunk_idx != chunk_id {
              index_cross_chunk_imports[chunk_id].insert(entry_chunk_idx);
              let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
              imports_from_other_chunks.entry(entry_chunk_idx).or_default();
            }
          }

          if self.options.preserve_modules {
            let entry_module =
              chunk.entry_module(&self.link_output.module_table).expect("Should have entry module");
            entry_module
              .import_records
              .iter()
              .filter(|rec| rec.kind != ImportKind::DynamicImport)
              .filter_map(|r| r.resolved_module)
              .for_each(|module_idx| {
                if !self.link_output.module_table[module_idx].side_effects().has_side_effects() {
                  return;
                }
                let Some(importee_chunk_idx) = chunk_graph.module_to_chunk[module_idx] else {
                  return;
                };
                index_cross_chunk_imports[chunk_id].insert(importee_chunk_idx);
                let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
                imports_from_other_chunks.entry(importee_chunk_idx).or_default();
              });
          }
        }

        // Add bare imports for side-effectful dependencies in other chunks. Under strict execution
        // order, only wrapped ESM importees are initialized by `init_*()` calls; unwrapped importees
        // still need the normal bare chunk import.
        let mut add_side_effect_imports_for_module = |module_idx: ModuleIdx| {
          let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
            return;
          };

          // From import records.
          // This adds side-effectful imports as bare imports if necessary.
          for rec in &module.import_records {
            if rec.kind != ImportKind::Import {
              continue;
            }
            let Some(importee_module_idx) = rec.resolved_module else {
              continue;
            };
            if self.options.is_strict_execution_order_enabled()
              && order_state
                .esm_init_target(importee_module_idx, &self.link_output.metas[importee_module_idx])
                .is_some()
            {
              continue;
            }
            if !self.link_output.module_table[importee_module_idx].side_effects().has_side_effects()
            {
              continue;
            }
            let Some(importee_chunk_idx) = chunk_graph.module_to_chunk[importee_module_idx] else {
              continue;
            };
            if importee_chunk_idx == chunk_id {
              continue;
            }
            index_cross_chunk_imports[chunk_id].insert(importee_chunk_idx);
            let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
            imports_from_other_chunks.entry(importee_chunk_idx).or_default();
          }

          // Runtime module may have side effects (e.g. dev/HMR mode) without an import record.
          if self.link_output.metas[module_idx].has_side_effectful_runtime_dep {
            let runtime_idx = self.link_output.runtime.id();
            if let Some(runtime_chunk_idx) = chunk_graph.module_to_chunk[runtime_idx] {
              if runtime_chunk_idx != chunk_id {
                index_cross_chunk_imports[chunk_id].insert(runtime_chunk_idx);
                let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
                imports_from_other_chunks.entry(runtime_chunk_idx).or_default();
              }
            }
          }
        };

        for &module_idx in &chunk.modules {
          add_side_effect_imports_for_module(module_idx);
        }

        // An order-wrap entry facade hosts no modules, but its prologue init call must still
        // run after the entry's side-effectful dependencies. Strict-gated to keep the flag-off
        // facade output identical to main.
        if self.options.is_strict_execution_order_enabled()
          && let ChunkKind::EntryPoint { module: entry_module_idx, .. } = &chunk.kind
          && !chunk.modules.contains(entry_module_idx)
        {
          add_side_effect_imports_for_module(*entry_module_idx);
        }
      });
  }

  fn deconflict_exported_names(
    &self,
    chunk_graph: &mut ChunkGraph,
    index_chunk_exported_symbols: &IndexChunkExportedSymbols,
    used_symbol_refs: &UsedSymbolRefs,
    order_live_symbols: &FxHashSet<SymbolRef>,
  ) {
    let is_preserve_modules_enabled = self.options.preserve_modules;
    let allow_to_minify_internal_exports =
      !is_preserve_modules_enabled && self.options.minify_internal_exports;
    // Generate cross-chunk exports. These must be computed before cross-chunk
    // imports because of export alias renaming, which must consider all export
    // aliases simultaneously to avoid collisions.
    let preserve_export_names_modules =
      std::mem::take(&mut chunk_graph.common_chunk_preserve_export_names_modules);
    for (chunk_id, chunk) in chunk_graph.chunk_table.iter_mut_enumerated() {
      if allow_to_minify_internal_exports {
        // Reference: https://github.com/rollup/rollup/blob/f76339428586620ff3e4c32fce48f923e7be7b05/src/utils/exportNames.ts#L5
        let mut named_index = 0;
        let mut used_names = FxHashSet::default();

        let mut processed_entry_exports = FxHashSet::default();
        if let Some(entry_module_idx) = chunk.entry_module_idx() {
          let exported_chunk_symbols = &index_chunk_exported_symbols[chunk_id];
          // If this's an entry point, we need to make sure the entry modules' exports are not minified.
          let entry_module = &self.link_output.metas[entry_module_idx];
          entry_module.canonical_exports(false).for_each(|(name, export)| {
            let export_ref = self.link_output.symbol_db.canonical_ref_for(export.symbol_ref);
            if !exported_chunk_symbols.contains_key(&export.symbol_ref)
              || !self.link_output.retained_export_symbols.contains(&export.symbol_ref)
            {
              // Rolldown supports tree-shaking on dynamic entries, so not all exports are used.
              return;
            }
            used_names.insert(name.clone());
            chunk.exports_to_other_chunks.entry(export_ref).or_default().push(name.clone());
            processed_entry_exports.insert(export_ref);
          });
        }
        // Also preserve exports from AllowExtension emitted chunks that were merged into this chunk
        if let Some(modules) = preserve_export_names_modules.get(&chunk_id) {
          let exported_chunk_symbols = &index_chunk_exported_symbols[chunk_id];
          for &module_idx in modules {
            let module_meta = &self.link_output.metas[module_idx];
            module_meta.canonical_exports(false).for_each(|(name, export)| {
              let export_ref = self.link_output.symbol_db.canonical_ref_for(export.symbol_ref);
              // Use canonical ref for lookup since that's the key in exported_chunk_symbols
              if !exported_chunk_symbols.contains_key(&export_ref)
                || !self.link_output.retained_export_symbols.contains(&export_ref)
              {
                return;
              }
              // Skip if already processed (e.g., same symbol re-exported from multiple modules)
              if processed_entry_exports.contains(&export_ref) {
                return;
              }
              used_names.insert(name.clone());
              chunk.exports_to_other_chunks.entry(export_ref).or_default().push(name.clone());
              processed_entry_exports.insert(export_ref);
            });
          }
        }
        for (chunk_export, _predefined_names) in index_chunk_exported_symbols[chunk_id]
          .iter()
          .sorted_unstable_by_key(|(symbol_ref, _predefined_names)| {
            // Canonical naming order — see `deconflict_order_key`.
            deconflict_order_key(
              **symbol_ref,
              &self.link_output.module_table,
              &self.link_output.symbol_db,
            )
          })
        {
          let export_ref = self.link_output.symbol_db.canonical_ref_for(*chunk_export);
          if processed_entry_exports.contains(&export_ref) {
            continue;
          }

          let mut export_name: CompactStr;
          loop {
            named_index += 1;
            export_name = generate_minified_names(named_index).into();
            if !used_names.contains(&export_name) {
              break;
            }
          }
          used_names.insert(export_name.clone());
          chunk.exports_to_other_chunks.entry(export_ref).or_default().push(export_name);
        }

        continue;
      }

      let mut resolver =
        ConflictResolver::with_capacity(index_chunk_exported_symbols[chunk_id].len());
      for (chunk_export, predefined_names) in index_chunk_exported_symbols[chunk_id]
        .iter()
        .sorted_by_cached_key(|(symbol_ref, _predefined_names)| {
          // Canonical naming order — see `deconflict_order_key`.
          deconflict_order_key(
            **symbol_ref,
            &self.link_output.module_table,
            &self.link_output.symbol_db,
          )
        })
      {
        // Same liveness rule as the cross-chunk import loop above (dynamic entries
        // register their namespace refs among the exported-symbol candidates).
        let is_live = if let Some(m) = self.link_output.module_table[chunk_export.owner].as_normal()
          && m.namespace_object_ref == *chunk_export
        {
          self.link_output.metas[chunk_export.owner].namespace_included
        } else {
          non_namespace_symbol_is_live(used_symbol_refs, order_live_symbols, *chunk_export)
        };
        if !is_live {
          continue;
        }
        let original_name: CompactStr = match predefined_names.as_slice() {
          [] => CompactStr::new(chunk_export.name(&self.link_output.symbol_db)),
          lst => {
            for item in lst {
              resolver.reserve(CompactStr::new(item));
            }

            chunk.exports_to_other_chunks.entry(*chunk_export).or_default().extend_from_slice(lst);
            continue;
          }
        };
        // A special case for `default` export when setting `preserve_modules`: the
        // single default export per chunk must be named `default`. Otherwise use the
        // `default_export_ref` representative name. The `&&` keeps the `entry_module`
        // lookup guarded behind the `preserve_modules` check.
        let base = if self.options.preserve_modules
          && chunk.entry_module(&self.link_output.module_table).unwrap().default_export_ref
            == *chunk_export
        {
          CompactStr::new_const("default")
        } else {
          original_name
        };
        let chosen = resolver.resolve(base, |_, _| true);
        chunk.exports_to_other_chunks.entry(*chunk_export).or_default().push(chosen);
      }
    }
  }
}

fn non_namespace_symbol_is_live(
  used_symbol_refs: &impl UsedSymbolRefsView,
  order_live_symbols: &FxHashSet<SymbolRef>,
  symbol_ref: SymbolRef,
) -> bool {
  used_symbol_refs.contains(&symbol_ref) || order_live_symbols.contains(&symbol_ref)
}

// The same implementation with https://github.com/oxc-project/oxc/blob/crates_v0.86.0/crates/oxc_mangler/src/base54.rs#L30-L31
const FIRST_BASE: u32 = 54;
const REST_BASE: u32 = 64;
const FREQUENT_CHARS: &[u8; REST_BASE as usize] =
  b"etnriaoscludfpmhg_vybxSCwTEDOkAjMNPFILRzBVHUWGKqJYXZQ$1024368579";

// Intentionally NOT routed through `ConflictResolver`: this is a generative
// base54 namer (not `$N`-suffix), so it shares only `deconflict_order_key`,
// not the conflict loop. See docs/superpowers/specs/2026-06-17-renamer-naming-engine-design.md.
fn generate_minified_names(mut value: u32) -> String {
  let mut buffer = vec![];

  // Base 54 at first because these are the usable first characters in JavaScript identifiers
  let byte = FREQUENT_CHARS[(value % FIRST_BASE) as usize];
  buffer.push(byte);
  value /= FIRST_BASE;

  while value > 0 {
    let byte = FREQUENT_CHARS[(value % REST_BASE) as usize];
    buffer.push(byte);
    value /= REST_BASE;
  }
  // SAFETY: `buffer` is base64 characters, it is valid utf8 characters
  unsafe { String::from_utf8_unchecked(buffer) }
}
