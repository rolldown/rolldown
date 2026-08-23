use super::{FinalEsmInitMetadata, GenerateStage, Sealed};
use crate::chunk_graph::ChunkGraph;
use crate::esm_init_obligations::{
  ObligationPurpose, WrappedEsmInitTarget, WrappedEsmInitTargetContext,
  collect_entry_reexported_wrapper_inits, collect_wrapped_esm_init_targets_for_import_record,
  for_each_init_obligation_record,
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
  PreserveEntrySignatures, RUNTIME_HELPER_NAMES, ResolvedImportRecord, RuntimeHelper, SymbolRef,
  SymbolRefDb, TaggedSymbolRef, UsedSymbolRefs, UsedSymbolRefsBuilder, UsedSymbolRefsView,
  WrapKind,
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
type IndexChunkDynamicImportsFromExternalModules = IndexVec<ChunkIdx, FxIndexSet<ModuleIdx>>;
type IndexImportsFromOtherChunks =
  IndexVec<ChunkIdx, FxHashMap<ChunkIdx, Vec<CrossChunkImportItem>>>;

/// A chunk is loaded through `import('./chunk.js')`, whose promise resolves with the chunk's
/// namespace object. Promise resolution assimilates any value with a callable `then` as a
/// thenable, so a chunk exporting `then` hijacks every dynamic import of that chunk — including
/// the `.then((n) => n.ns)` the finalizer generates to reach a merged dynamic entry, whose
/// callback then receives whatever the exported `then` resolved with instead of the namespace.
///
/// Internal cross-chunk export names are bundler-owned, so simply never hand out `then` for one.
/// Names the user can observe — an entry chunk's public exports and the export names an
/// `emitFile` consumer relies on — are a contract and keep whatever they declare.
const THENABLE_HAZARD_EXPORT_NAME: &str = "then";

struct CrossChunkLinkState {
  index_chunk_exported_symbols: IndexChunkExportedSymbols,
  index_chunk_direct_imports_from_external_modules: IndexChunkImportsFromExternalModules,
  index_chunk_indirect_imports_from_external_modules: IndexChunkAllImportsFromExternalModules,
  index_imports_from_other_chunks: IndexImportsFromOtherChunks,
  index_cross_chunk_imports: IndexCrossChunkImports,
  index_cross_chunk_dynamic_imports: IndexCrossChunkDynamicImports,
  index_chunk_dynamic_imports_from_external_modules: IndexChunkDynamicImportsFromExternalModules,
  order_live_symbols: FxHashSet<SymbolRef>,
  symbol_chunk_table: SymbolChunkTable,
}

#[derive(Clone, Copy)]
enum FinalEsmInitMetadataAvailability<'a> {
  /// The prediction pass runs before wrapper selection and final chunk topology are fixed.
  Unavailable,
  /// Final cross-chunk linking can only receive metadata through the sealed boundary.
  Sealed(&'a Sealed<FinalEsmInitMetadata>),
}

impl<'a> FinalEsmInitMetadataAvailability<'a> {
  fn sealed(self) -> Option<&'a Sealed<FinalEsmInitMetadata>> {
    match self {
      Self::Unavailable => None,
      Self::Sealed(metadata) => Some(metadata),
    }
  }
}

/// Symbol -> owning chunk, derived by [`GenerateStage::collect_depended_symbols`] for one
/// cross-chunk link pass.
///
/// The derivation used to be written into the shared symbol database mid-pass so that
/// [`GenerateStage::compute_chunk_imports`] could read it back — a load-bearing write that forced
/// every link pass, including the what-if ones, to mutate shared state and undo it afterwards.
/// Carried as pass-local data instead, the whole pass is read-only by construction: a pass whose
/// result is never committed (the prediction pass, the entry-facade edge query) simply drops its
/// table, and the final [`GenerateStage::compute_cross_chunk_links`] remains the single writer,
/// flushing its table into the database in one place for the downstream consumers of
/// `chunk_idx` (the CJS cross-chunk reference rendering in the module finalizer and the
/// chunk-export generator).
struct SymbolChunkTable {
  map: FxHashMap<SymbolRef, ChunkIdx>,
}

impl SymbolChunkTable {
  /// The chunk owning `symbol_ref`, as this pass derived it; falls back to the value already in
  /// the symbol database, mirroring the write-then-read behavior this table replaced (a symbol
  /// the pass did not assign resolves exactly as it did then).
  fn chunk_of(&self, symbol_ref: SymbolRef, symbols: &SymbolRefDb) -> Option<ChunkIdx> {
    self.map.get(&symbol_ref).copied().or_else(|| symbols.get(symbol_ref).chunk_idx)
  }
}

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn compute_cross_chunk_links(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
  ) {
    let CrossChunkLinkState {
      index_chunk_exported_symbols,
      index_chunk_direct_imports_from_external_modules,
      mut index_chunk_indirect_imports_from_external_modules,
      index_imports_from_other_chunks,
      index_cross_chunk_imports,
      index_cross_chunk_dynamic_imports,
      index_chunk_dynamic_imports_from_external_modules,
      order_live_symbols,
      symbol_chunk_table,
    } = self.compute_cross_chunk_link_state(
      chunk_graph,
      used_symbol_refs.view(),
      order_state,
      FinalEsmInitMetadataAvailability::Sealed(final_esm_init_metadata),
    );
    // The single flush of symbol->chunk ownership into the shared symbol database. Everything
    // downstream that reads `chunk_idx` — the module finalizer and the chunk-export generator
    // rendering CJS cross-chunk references — sees exactly this pass's derivation; the what-if
    // passes (prediction, the entry-facade edge query) never write at all.
    self.commit_symbol_chunk_table(&symbol_chunk_table);

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

    #[cfg(debug_assertions)]
    self.debug_assert_module_level_static_import_prediction(
      chunk_graph,
      used_symbol_refs.view(),
      &index_imports_from_other_chunks,
    );

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
      index_chunk_dynamic_imports_from_external_modules,
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
        dynamic_imports_from_external_modules,
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
        chunk.dynamic_imports_from_external_modules =
          dynamic_imports_from_external_modules.into_iter().collect::<Vec<_>>();
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
  }

  /// Compute provisional links for order analysis. Uses an empty order state and explicitly marks
  /// final-init metadata unavailable, so the edges are the *pre-lowering* baseline topology (value
  /// and side-effect imports, before wrapping adds `init_*` imports). The emergent-cycle fixpoint
  /// layers the plan's `init_*` forwarding edges on top of this baseline
  /// (`post_lowering_import_edges`). Read-only: the symbol ownership this pass derives is dropped
  /// with its table, so no provisional values ever reach the symbol database.
  pub(super) fn predicted_static_import_edges(
    &self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs_builder: &UsedSymbolRefsBuilder,
  ) -> IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> {
    let empty_order_state = super::order_wrap_state::OrderWrapState::default();
    let state = self.compute_cross_chunk_link_state(
      chunk_graph,
      used_symbol_refs_builder.view(),
      &empty_order_state,
      FinalEsmInitMetadataAvailability::Unavailable,
    );
    Self::static_import_edges_of(chunk_graph, state)
  }

  /// The static chunk->chunk import edges the *final* cross-chunk link pass will compute, asked
  /// before that pass runs.
  ///
  /// Unlike [`Self::predicted_static_import_edges`] this is not a prediction: it drives the same
  /// machinery with the fully lowered [`OrderWrapState`](super::order_wrap_state::OrderWrapState)
  /// and the real final init metadata, so every edge source the final pass registers — value
  /// references, `init_*` forwarding, retained re-export overlays, transitive init obligations —
  /// is present, with no re-derived approximation that could silently miss one.
  ///
  /// The entry-facade decision in `create_order_wrap_entry_facades` needs exactly this: an entry's
  /// inline `init_E()` trigger is only safe while nothing *else* loads the chunk hosting `E`.
  ///
  /// Answering it here is sound because the facade split cannot invalidate the answer. A facade is
  /// an empty chunk that takes over the entry role from the implementation chunk: it adds one
  /// importer of that implementation chunk (itself), and it inherits — never widens — the outgoing
  /// edges the implementation chunk had as an entry chunk. So for every *other* chunk the relation
  /// "some chunk other than me imports me" is unchanged, and all facade decisions can be taken
  /// together from this one pre-facade snapshot.
  ///
  /// The symbol->chunk ownership the pass derives stays in its pass-local [`SymbolChunkTable`] and
  /// is dropped with the state, so this query is read-only by construction and the final pass is
  /// still the only writer of the symbol database.
  pub(super) fn lowered_static_import_edges(
    &self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs_builder: &UsedSymbolRefsBuilder,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
  ) -> IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> {
    let state = self.compute_cross_chunk_link_state(
      chunk_graph,
      used_symbol_refs_builder.view(),
      order_state,
      FinalEsmInitMetadataAvailability::Sealed(final_esm_init_metadata),
    );
    Self::static_import_edges_of(chunk_graph, state)
  }

  /// Reconciles the module-level prediction (`predicted_static_import_targets` plus
  /// `entry_export_service_targets`) against the static chunk imports this pass just derived.
  /// The already-loaded fold consumed that prediction before chunks existed, with two
  /// obligations: the fold's cycle check must see every emitted edge, and its entry reachability
  /// must not claim a side effect the emitted graph never runs. The contract is therefore
  /// one-and-a-half-sided:
  /// - every edge this pass derives must be predicted (a miss is the cycle-check bug class
  ///   pinned by `optimization/chunk_merging/already_loaded_entry_reexport_service_edge`),
  ///   except edges whose every import item is a runtime-owned symbol: per-chunk runtime-helper
  ///   demands and CJS-format interop request those at emission time and the module-level walk
  ///   does not model them;
  /// - a predicted *entry-export service* edge with a side-effectful target must be emitted.
  ///   Those targets exceed `load_dependencies`, so nothing but their liveness gate keeps entry
  ///   reachability truthful; this direction validates that the gate mirrors emission. The base
  ///   prediction is exempt from this direction: it filters `load_dependencies`, the same edges
  ///   the pre-prediction bits reachability trusted, so its over-predictions are exactly
  ///   main-parity — e.g. constant inlining drops a symbol's import (and can even orphan an
  ///   annotated-side-effectful chunk, see `rollup@chunking form@namespace-reexport-side-effect-cache`)
  ///   without touching `load_dependencies`, on main just as here.
  ///
  /// Service imports hang on the entry (possibly facade) chunk, so entry chunks predict them via
  /// their `entry_module_idx` — hosted or not — plus the facade edge to the chunk hosting a
  /// moved-away entry module.
  ///
  /// Scope: the hard direction covers the edges derived here (`index_imports_from_other_chunks`).
  /// Edges pre-populated on `Chunk::imports_from_other_chunks` by chunk merging postdate the fold
  /// and answer to `would_create_circular_dependency`, not to this prediction; they only join the
  /// union used by the soft direction. This validates the prediction *function* on the final
  /// state — the fold's decision-time snapshot predates the inclusion replays, which is why the
  /// fold's own cycle graph over-approximates liveness instead of trusting it.
  #[cfg(debug_assertions)]
  fn debug_assert_module_level_static_import_prediction(
    &self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs_view: UsedSymbolRefsView<'_>,
    index_imports_from_other_chunks: &IndexImportsFromOtherChunks,
  ) {
    if self.options.is_strict_execution_order_enabled() || self.options.preserve_modules {
      return;
    }
    let runtime_idx = self.link_output.runtime.id();
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      // A chunk with no modules is either a live entry facade or a husk left behind by facade
      // elimination, runtime-chunk merging, or dynamic-entry absorption; the two are not
      // distinguishable here and a husk gets no emission edges at all. Module-level prediction
      // has nothing to say about either (their only edges are the emission-owned facade and
      // service imports), so they are out of this contract's scope.
      if chunk.modules.is_empty() {
        continue;
      }
      // Predicted edge -> whether a side-effectful target reaches it through the service
      // extension (the only component held to the soft direction).
      let mut predicted: FxHashMap<ChunkIdx, bool> = FxHashMap::default();
      let note_target =
        |target: ModuleIdx, from_service: bool, predicted: &mut FxHashMap<ChunkIdx, bool>| {
          if let Some(target_chunk_idx) = chunk_graph.module_to_chunk[target]
            && target_chunk_idx != chunk_idx
          {
            let side_effectful = from_service
              && self.link_output.module_table[target].side_effects().has_side_effects();
            *predicted.entry(target_chunk_idx).or_insert(false) |= side_effectful;
          }
        };
      for &module_idx in &chunk.modules {
        for target in self.predicted_static_import_targets(module_idx) {
          note_target(target, false, &mut predicted);
        }
      }
      if let ChunkKind::EntryPoint { module: entry_module_idx, .. } = chunk.kind {
        let mut service_targets = vec![];
        self.entry_export_service_targets(
          entry_module_idx,
          used_symbol_refs_view,
          false,
          &mut service_targets,
        );
        for target in service_targets {
          note_target(target, true, &mut predicted);
        }
        if !chunk.modules.contains(&entry_module_idx) {
          // The facade runs its moved-away entry module by importing the chunk hosting it.
          if let Some(host_chunk_idx) = chunk_graph.module_to_chunk[entry_module_idx]
            && host_chunk_idx != chunk_idx
          {
            predicted.entry(host_chunk_idx).or_insert(false);
          }
        }
      }
      for (importee_chunk_idx, items) in &index_imports_from_other_chunks[chunk_idx] {
        let runtime_requested_only =
          !items.is_empty() && items.iter().all(|item| item.import_ref.owner == runtime_idx);
        debug_assert!(
          runtime_requested_only || predicted.contains_key(importee_chunk_idx),
          "emitted static import {chunk_idx:?} -> {importee_chunk_idx:?} was not predicted; the \
           already-loaded cycle check ran without this edge",
        );
      }
      for (importee_chunk_idx, side_effectful_service_target) in predicted {
        debug_assert!(
          !side_effectful_service_target
            || index_imports_from_other_chunks[chunk_idx].contains_key(&importee_chunk_idx)
            || chunk.imports_from_other_chunks.contains_key(&importee_chunk_idx),
          "predicted side-effectful service import {chunk_idx:?} -> {importee_chunk_idx:?} was \
           not emitted; the liveness gate diverged from emission and already-loaded reachability \
           may claim a side effect that never runs",
        );
      }
    }
  }

  fn static_import_edges_of(
    chunk_graph: &ChunkGraph,
    state: CrossChunkLinkState,
  ) -> IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> {
    state
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
    &self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs_view: UsedSymbolRefsView<'_>,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: FinalEsmInitMetadataAvailability<'_>,
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
    let mut index_chunk_dynamic_imports_from_external_modules:
      IndexChunkDynamicImportsFromExternalModules =
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

    let symbol_chunk_table = self.collect_depended_symbols(
      chunk_graph,
      &mut index_chunk_depended_symbols,
      &mut index_chunk_direct_imports_from_external_modules,
      &mut index_cross_chunk_dynamic_imports,
      &mut index_chunk_dynamic_imports_from_external_modules,
      used_symbol_refs_view,
      order_state,
      final_esm_init_metadata,
    );

    self.compute_chunk_imports(
      chunk_graph,
      &index_chunk_depended_symbols,
      &index_chunk_direct_imports_from_external_modules,
      &mut index_chunk_exported_symbols,
      &mut index_cross_chunk_imports,
      &mut index_imports_from_other_chunks,
      &mut index_chunk_indirect_imports_from_external_modules,
      used_symbol_refs_view,
      order_state,
      &order_live_symbols,
      &symbol_chunk_table,
    );

    CrossChunkLinkState {
      index_chunk_exported_symbols,
      index_chunk_direct_imports_from_external_modules,
      index_chunk_indirect_imports_from_external_modules,
      index_imports_from_other_chunks,
      index_cross_chunk_imports,
      index_cross_chunk_dynamic_imports,
      index_chunk_dynamic_imports_from_external_modules,
      order_live_symbols,
      symbol_chunk_table,
    }
  }

  fn collect_external_import(
    &self,
    importer_idx: ModuleIdx,
    import_record: &ResolvedImportRecord,
    external_module_idx: ModuleIdx,
    imports_from_external_modules: &mut FxHashMap<ModuleIdx, Vec<(ModuleIdx, NamedImport)>>,
    dynamic_imports_from_external_modules: &mut FxIndexSet<ModuleIdx>,
  ) {
    if matches!(import_record.kind, ImportKind::DynamicImport)
      && import_record.dynamic_import_expr_info.as_ref().is_none_or(|info| {
        self.link_output.metas[importer_idx].stmt_info_included.has_bit(info.stmt_info_idx)
      })
    {
      dynamic_imports_from_external_modules.insert(external_module_idx);
    }
    // Ensure the external module is imported in case it has side effects.
    if matches!(import_record.kind, ImportKind::Import)
      && !import_record.meta.contains(ImportRecordMeta::IsExportStar)
    {
      imports_from_external_modules.entry(external_module_idx).or_default();
    }
  }

  fn collect_dynamic_chunk_import(
    &self,
    chunk_graph: &ChunkGraph,
    import_record: &ResolvedImportRecord,
    importee_module_idx: ModuleIdx,
    cross_chunk_dynamic_imports: &mut FxIndexSet<ChunkIdx>,
  ) {
    // The resolved module is not included in the module graph, skip it.
    if !self.link_output.metas[importee_module_idx].is_included
      || !matches!(import_record.kind, ImportKind::DynamicImport)
    {
      return;
    }
    // The finalizer rewrites `import()` specifiers through `entry_module_to_entry_chunk`, which
    // diverges from the hosting chunk whenever the dynamic entry's facade chunk survives while
    // another chunk hosts its body (order-wrap facade splits, or kept facades when common-chunk
    // merging is off); record the chunk the emitted specifier actually names.
    let importee_chunk = chunk_graph
      .entry_module_to_entry_chunk
      .get(&importee_module_idx)
      .copied()
      .or(chunk_graph.module_to_chunk[importee_module_idx])
      .expect("importee chunk should exist");
    cross_chunk_dynamic_imports.insert(importee_chunk);
  }

  /// - Derive each declared symbol's owning chunk, returned as the pass-local
  ///   [`SymbolChunkTable`]
  /// - Collect all referenced symbols and consider them potential imports
  #[expect(clippy::too_many_arguments)]
  fn collect_depended_symbols(
    &self,
    chunk_graph: &ChunkGraph,
    index_chunk_depended_symbols: &mut IndexChunkDependedSymbols,
    index_chunk_imports_from_external_modules: &mut IndexChunkImportsFromExternalModules,
    index_cross_chunk_dynamic_imports: &mut IndexCrossChunkDynamicImports,
    index_chunk_dynamic_imports_from_external_modules: &mut IndexChunkDynamicImportsFromExternalModules,
    used_symbol_refs_view: UsedSymbolRefsView<'_>,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: FinalEsmInitMetadataAvailability<'_>,
  ) -> SymbolChunkTable {
    let symbols = &self.link_output.symbol_db;
    let chunk_id_to_symbols_vec = append_only_vec::AppendOnlyVec::new();

    let chunks_iter = multizip((
      chunk_graph.chunk_table.iter_enumerated(),
      index_chunk_depended_symbols.iter_mut(),
      index_chunk_imports_from_external_modules.iter_mut(),
      index_cross_chunk_dynamic_imports.iter_mut(),
      index_chunk_dynamic_imports_from_external_modules.iter_mut(),
    ));

    chunks_iter.par_bridge().for_each(
      |(
        (chunk_id, chunk),
        depended_symbols,
        imports_from_external_modules,
        cross_chunk_dynamic_imports,
        dynamic_imports_from_external_modules,
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
            .for_each(|(rec, module_idx)| match &self.link_output.module_table[module_idx] {
              Module::Normal(_) => self.collect_dynamic_chunk_import(
                chunk_graph,
                rec,
                module_idx,
                cross_chunk_dynamic_imports,
              ),
              Module::External(_) => self.collect_external_import(
                module.idx,
                rec,
                module_idx,
                imports_from_external_modules,
                dynamic_imports_from_external_modules,
              ),
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
              if stmt_info.import_records.iter().any(|rec_idx| {
                order_state.has_order_cjs_carrier(super::order_wrap_state::OrderCjsCarrierKey {
                  importer: module.idx,
                  record: *rec_idx,
                })
              }) {
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
            used_symbol_refs_view,
            order_state,
            final_esm_init_metadata,
            depended_symbols,
            module.idx,
          );
        });

        if let Some(entry_id) = &chunk.entry_module_idx() {
          let entry = &self.link_output.module_table[*entry_id].as_normal().unwrap();
          let entry_meta = &self.link_output.metas[entry.idx];

          if !matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
            self.register_entry_export_depended_symbols(
              chunk_graph,
              order_state,
              depended_symbols,
              entry.idx,
              entry_meta,
            );
          }

          if matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
            depended_symbols
              .insert(entry_meta.wrapper_ref.expect("CJS entry should have a wrapper"));
          } else if let Some(targets) = order_state.consumer_local_namespace_targets(entry.idx) {
            for &target in targets {
              self.add_wrapped_esm_init_target_depended_symbol(
                chunk_graph,
                order_state,
                depended_symbols,
                target,
              );
            }
          } else if let Some(target) = order_state.esm_init_target(entry.idx, entry_meta) {
            depended_symbols.insert(target.wrapper_ref);
          }
          if let Some(final_esm_init_metadata) = final_esm_init_metadata.sealed() {
            self.add_transitive_esm_init_depended_symbols(
              chunk_graph,
              order_state,
              final_esm_init_metadata,
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

        self.add_absorbed_entry_init_deps(chunk_graph, order_state, depended_symbols, chunk_id);

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
    self.build_symbol_chunk_table(chunk_id_to_symbols_vec)
  }

  /// Record which chunk owns each declared symbol, so [`Self::compute_chunk_imports`] can resolve a
  /// depended symbol to the chunk it must be imported from.
  fn build_symbol_chunk_table(
    &self,
    chunk_id_to_symbols_vec: append_only_vec::AppendOnlyVec<(ChunkIdx, Vec<TaggedSymbolRef>)>,
  ) -> SymbolChunkTable {
    let mut map = FxHashMap::default();
    for (chunk_idx, symbol_list) in chunk_id_to_symbols_vec {
      for declared in symbol_list {
        let declared = declared.inner();
        let previous = map.insert(declared, chunk_idx);
        debug_assert!(
          previous.is_none_or(|previous| previous == chunk_idx),
          "Symbol: {:?}, {:?} in {:?} should only belong to one chunk. Existed {previous:?}, new {chunk_idx:?}",
          declared.name(&self.link_output.symbol_db),
          declared,
          self.link_output.module_table[declared.owner].id().as_str(),
        );
      }
    }
    SymbolChunkTable { map }
  }

  /// The single point where derived symbol->chunk ownership enters the shared symbol database.
  fn commit_symbol_chunk_table(&mut self, table: &SymbolChunkTable) {
    let symbols = &mut self.link_output.symbol_db;
    for (symbol_ref, chunk_idx) in &table.map {
      symbols.get_mut(*symbol_ref).chunk_idx = Some(*chunk_idx);
    }
  }

  /// Register what a non-CJS entry's export signature makes the chunk depend on: the canonical
  /// symbol of every re-exported binding, plus — off-strict — the `init_*` wrapper of every
  /// ESM-wrapped module backing one. The wrappers come from the same walk entry emission
  /// consumes (`collect_entry_reexported_wrapper_inits`), so everything the entry chunk may call
  /// is imported by construction; `None` for the names because registration runs before chunk
  /// names exist — it is what makes a wrapper reachable. The entry's own wrapper is not part of
  /// the walk; the caller's `esm_init_target` arm covers it.
  fn register_entry_export_depended_symbols(
    &self,
    chunk_graph: &ChunkGraph,
    order_state: &super::order_wrap_state::OrderWrapState,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    entry_idx: ModuleIdx,
    entry_meta: &crate::types::linking_metadata::LinkingMetadata,
  ) {
    let symbols = &self.link_output.symbol_db;
    let sorted_export_refs = entry_meta
      .resolved_exports
      .iter()
      .sorted_unstable_by_key(|(name, _)| *name)
      .map(|(_, export)| export)
      // A chunk should always consume a cjs export symbol by property access, so filter
      // out a exported symbol that came from a cjs module.
      .filter(|resolved_export| !resolved_export.came_from_commonjs);
    if self.options.is_strict_execution_order_enabled() {
      for export_ref in sorted_export_refs {
        self.add_depended_symbol_with_wrapped_esm_init(
          chunk_graph,
          order_state,
          depended_symbols,
          symbols.canonical_ref_resolving_namespace(export_ref.symbol_ref),
        );
      }
    } else {
      for export_ref in sorted_export_refs {
        depended_symbols.insert(symbols.canonical_ref_resolving_namespace(export_ref.symbol_ref));
      }
      for init in collect_entry_reexported_wrapper_inits(
        entry_idx,
        entry_meta,
        &self.link_output.metas,
        &self.link_output.module_table.modules,
        symbols,
        None,
      ) {
        depended_symbols.insert(init.wrapper_ref);
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

    // A carrier namespace is declared beside its CJS importee, even though the synthetic symbol
    // is owned by the forwarding barrel. Register that exact per-record carrier as an additional
    // init companion before the ordinary owner-based path below considers the barrel wrapper.
    if let Some(key) = order_state.order_cjs_carrier_key_for_namespace(symbol_ref) {
      self.add_wrapped_esm_init_target_depended_symbol(
        chunk_graph,
        order_state,
        depended_symbols,
        WrappedEsmInitTarget::CjsCarrier(key),
      );
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
    used_symbol_refs_view: UsedSymbolRefsView<'_>,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: FinalEsmInitMetadataAvailability<'_>,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    module_idx: ModuleIdx,
  ) {
    if let Some(final_esm_init_metadata) = final_esm_init_metadata.sealed() {
      self.add_transitive_esm_init_depended_symbols(
        chunk_graph,
        order_state,
        final_esm_init_metadata,
        depended_symbols,
        module_idx,
      );
    }
    self.add_included_import_esm_init_depended_symbols(
      chunk_graph,
      used_symbol_refs_view,
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
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    module_idx: ModuleIdx,
  ) {
    let Some(targets_by_stmt) = final_esm_init_metadata.transitive_init_targets(module_idx) else {
      return;
    };
    // Iterate the targets in a deterministic, cross-target-stable order. The map is an
    // `FxHashMap<StmtInfoIdx, _>`, and its iteration order follows FxHash bucket layout. FxHash is
    // unseeded but hashes differently on 32-bit vs 64-bit, so iteration visits buckets in a
    // different order on native (64-bit) than on wasm32/WASI. That order flows straight into
    // `depended_symbols` (an `FxIndexSet`), whose insertion order drives the chunk's imported-symbol
    // rename order (the `$1`/`$2` suffixes) in `deconflict_chunk_symbols` — so a hash-ordered walk
    // here makes native and WASI builds resolve rename collisions differently. Sorting by the owning
    // `StmtInfoIdx` pins one order for every target.
    for (_, targets) in
      targets_by_stmt.iter().sorted_unstable_by_key(|(stmt_info_idx, _)| **stmt_info_idx)
    {
      for &target in targets {
        self.add_wrapped_esm_init_target_depended_symbol(
          chunk_graph,
          order_state,
          depended_symbols,
          target,
        );
      }
    }
  }

  fn add_wrapped_esm_init_target_depended_symbol(
    &self,
    chunk_graph: &ChunkGraph,
    order_state: &super::order_wrap_state::OrderWrapState,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    target: WrappedEsmInitTarget,
  ) {
    match target {
      WrappedEsmInitTarget::Module(target_idx) => {
        let meta = &self.link_output.metas[target_idx];
        if let Some(target) = order_state.esm_init_target(target_idx, meta)
          && order_state.init_target_included_in_live_chunk(&target, meta, target_idx, chunk_graph)
        {
          depended_symbols.insert(target.wrapper_ref);
        }
      }
      WrappedEsmInitTarget::CjsCarrier(key) => {
        if order_state.order_cjs_carrier_included_in_live_chunk(key, chunk_graph)
          && let Some(carrier) = order_state.order_cjs_carrier(key)
        {
          depended_symbols.insert(carrier.wrapper_ref);
        }
      }
    }
  }

  /// A collapsed dynamic-entry facade runs its initialization at each `import()` call site.
  /// Consumer-local barrels replace the entry's shared wrapper with the complete namespace target
  /// list, which can contain leaf wrappers and CJS carriers hosted by other chunks. Register those
  /// targets as dependencies of the absorbed entry's host chunk so both the same-chunk direct calls
  /// and the cross-chunk re-exports have real backing imports.
  fn add_absorbed_entry_init_deps(
    &self,
    chunk_graph: &ChunkGraph,
    order_state: &super::order_wrap_state::OrderWrapState,
    depended_symbols: &mut FxIndexSet<SymbolRef>,
    chunk_idx: ChunkIdx,
  ) {
    let Some(dynamic_entries) =
      chunk_graph.common_chunk_exported_facade_chunk_namespace.get(&chunk_idx)
    else {
      return;
    };
    for dynamic_entry in dynamic_entries {
      if let Some(targets) = order_state.consumer_local_namespace_targets(*dynamic_entry) {
        for &target in targets {
          self.add_wrapped_esm_init_target_depended_symbol(
            chunk_graph,
            order_state,
            depended_symbols,
            target,
          );
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
    used_symbol_refs_view: UsedSymbolRefsView<'_>,
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
          |symbol_ref| used_symbol_refs_view.contains(&symbol_ref),
          |_| true,
          |forwarding_module_idx| {
            chunk_graph.module_to_chunk[forwarding_module_idx] == Some(chunk_idx)
          },
        );
        for target in targets {
          self.add_wrapped_esm_init_target_depended_symbol(
            chunk_graph,
            order_state,
            depended_symbols,
            target,
          );
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
    used_symbol_refs_view: UsedSymbolRefsView<'_>,
    order_state: &super::order_wrap_state::OrderWrapState,
    order_live_symbols: &FxHashSet<SymbolRef>,
    symbol_chunk_table: &SymbolChunkTable,
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
            } else if let Some(targets) =
              order_state.consumer_local_namespace_targets(*dynamic_entry_module)
            {
              // A consumer-local namespace is activated by its complete leaf/carrier target list,
              // never by the intentionally empty shared barrel wrapper.
              for &target in targets {
                let wrapper_ref = match target {
                  WrappedEsmInitTarget::Module(module_idx) => {
                    order_state
                      .esm_init_target(module_idx, &self.link_output.metas[module_idx])
                      .expect("dynamic-entry module target should have a wrapper")
                      .wrapper_ref
                  }
                  WrappedEsmInitTarget::CjsCarrier(key) => {
                    order_state
                      .order_cjs_carrier(key)
                      .expect("dynamic-entry CJS carrier should have a wrapper")
                      .wrapper_ref
                  }
                };
                index_chunk_exported_symbols[chunk_id].entry(wrapper_ref).or_default();
              }
              let ns_ref = self.link_output.module_table[*dynamic_entry_module]
                .namespace_object_ref()
                .expect("dynamic entry should be normal module");
              index_chunk_exported_symbols[chunk_id].entry(ns_ref).or_default();
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
            non_namespace_symbol_is_live(used_symbol_refs_view, order_live_symbols, import_ref)
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
            if symbol_chunk_table.chunk_of(to_esm_ref, &self.link_output.symbol_db).is_some() {
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
          let importee_chunk_idx = symbol_chunk_table
            .chunk_of(import_ref, &self.link_output.symbol_db)
            .unwrap_or_else(|| {
              let symbol_owner = &self.link_output.module_table[import_ref.owner];
              let symbol_name = import_ref.name(&self.link_output.symbol_db);
              panic!(
                "Symbol `{}` in `{}` should belong to a chunk",
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
            export_name = generate_minified_names(named_index);
            // Unreachable in practice — the generator first produces the four-character `then`
            // at value 443,179, i.e. after ~443k internal exports in one chunk — but it is the
            // only other source of internal export names, so make it impossible rather than
            // improbable.
            if !used_names.contains(&export_name) && export_name != THENABLE_HAZARD_EXPORT_NAME {
              break;
            }
          }
          used_names.insert(export_name.clone());
          chunk.exports_to_other_chunks.entry(export_ref).or_default().push(export_name);
        }

        continue;
      }

      // The symbols an `emitFile` consumer reaches under the name `then`. Unlike an entry
      // signature they arrive with no predefined name, so they would otherwise be
      // indistinguishable from a bundler-owned internal name below. Only the export name is the
      // contract: a preserve-name module whose local `then` leaves under an alias goes through
      // the resolver like everything else, so it can never hand `then` to the chunk.
      //
      // The carve-out below additionally requires the declaring symbol to be named `then`: with
      // internal minification off this pass outputs declaring-symbol names, so export aliases are
      // already not honored — a pre-existing defect tracked in #10500, whose fix (routing
      // preserved names through the predefined-names path) also removes this set and the
      // first-wins flag below.
      let preserved_then_refs: FxHashSet<SymbolRef> = preserve_export_names_modules
        .get(&chunk_id)
        .map(|modules| {
          modules
            .iter()
            .flat_map(|&module_idx| {
              self.link_output.metas[module_idx]
                .canonical_exports(false)
                .filter(|(name, _)| name.as_str() == THENABLE_HAZARD_EXPORT_NAME)
                .map(|(_, export)| self.link_output.symbol_db.canonical_ref_for(export.symbol_ref))
            })
            .collect()
        })
        .unwrap_or_default();
      let mut preserved_then_taken = false;

      let mut resolver =
        ConflictResolver::with_capacity(index_chunk_exported_symbols[chunk_id].len());
      // Names taken from source symbols can collide with `then`; reserving it up front deconflicts
      // those to `then$1` like any other collision. See
      // internal-docs/code-splitting/design.md ("Thenable chunk namespaces"). Predefined names take the `lst` branch below,
      // which re-reserves (a no-op) and emits them verbatim, so a public `then` still stays `then`.
      resolver.reserve(CompactStr::new_const(THENABLE_HAZARD_EXPORT_NAME));
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
          non_namespace_symbol_is_live(used_symbol_refs.view(), order_live_symbols, *chunk_export)
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
        //
        // `preserve_modules` emits one chunk per module, so a chunk normally carries the module it
        // mirrors. Synthetic chunks are the exception: the shared `rolldown-runtime` chunk that
        // strict execution order splits out mirrors no user module and is `ChunkKind::Common`, so
        // it has no entry module and none of its exports can be a module's default export.
        let base = if self.options.preserve_modules
          && chunk
            .entry_module(&self.link_output.module_table)
            .is_some_and(|entry_module| entry_module.default_export_ref == *chunk_export)
        {
          CompactStr::new_const("default")
        } else {
          original_name
        };
        let chosen = if base == THENABLE_HAZARD_EXPORT_NAME
          && !preserved_then_taken
          && preserved_then_refs
            .contains(&self.link_output.symbol_db.canonical_ref_for(*chunk_export))
        {
          // A name an `emitFile` consumer relies on is a contract, so it keeps `then`. The
          // reservation above only fences off bundler-owned internal names. Two preserved
          // modules both exporting the name `then` cannot both be honored — the second falls
          // back to the resolver so the chunk at least stays parseable.
          preserved_then_taken = true;
          base
        } else {
          resolver.resolve(base, |_, _| true)
        };
        chunk.exports_to_other_chunks.entry(*chunk_export).or_default().push(chosen);
      }
    }
  }
}

fn non_namespace_symbol_is_live(
  used_symbol_refs_view: UsedSymbolRefsView<'_>,
  order_live_symbols: &FxHashSet<SymbolRef>,
  symbol_ref: SymbolRef,
) -> bool {
  used_symbol_refs_view.contains(&symbol_ref) || order_live_symbols.contains(&symbol_ref)
}

// The same implementation with https://github.com/oxc-project/oxc/blob/crates_v0.86.0/crates/oxc_mangler/src/base54.rs#L30-L31
const FIRST_BASE: u32 = 54;
const REST_BASE: u32 = 64;
const FREQUENT_CHARS: &[u8; REST_BASE as usize] =
  b"etnriaoscludfpmhg_vybxSCwTEDOkAjMNPFILRzBVHUWGKqJYXZQ$1024368579";

// Intentionally NOT routed through `ConflictResolver`. This is a generative base54 namer, not
// a `$N`-suffix one. Its call site shares `deconflict_order_key` with the resolver path, but
// not the conflict loop (#9831).
fn generate_minified_names(mut value: u32) -> CompactStr {
  // `u32::MAX` needs 6 bytes: one base-54 head plus five base-64 digits, because
  // `u32::MAX / FIRST_BASE` lands between `REST_BASE.pow(4)` and `REST_BASE.pow(5)`.
  let mut buffer = [0u8; 6];
  let mut len = 0;

  // Base 54 at first because these are the usable first characters in JavaScript identifiers
  buffer[len] = FREQUENT_CHARS[(value % FIRST_BASE) as usize];
  len += 1;
  value /= FIRST_BASE;

  while value > 0 {
    buffer[len] = FREQUENT_CHARS[(value % REST_BASE) as usize];
    len += 1;
    value /= REST_BASE;
  }
  // SAFETY: every byte written comes from `FREQUENT_CHARS`, which is ASCII.
  CompactStr::new(unsafe { std::str::from_utf8_unchecked(&buffer[..len]) })
}
