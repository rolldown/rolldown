//! The namespace that `import()` gives for a pure dynamic entry chunk.
//!
//! See internal-docs/code-splitting/implementation.md#dynamic-entry-namespaces.

use rolldown_common::{Chunk, ChunkIdx, ChunkKind, ChunkMeta, ModuleIdx, UsedSymbolRefsBuilder};
#[cfg(debug_assertions)]
use rolldown_common::{PreserveEntrySignatures, UsedSymbolRefsView, WrapKind};
use rustc_hash::FxHashSet;

use crate::chunk_graph::ChunkGraph;
#[cfg(debug_assertions)]
use crate::utils::chunk::normalize_preserve_entry_signature;

#[cfg(debug_assertions)]
use super::compute_cross_chunk_links::CrossChunkLinkState;
use super::{
  GenerateStage, compute_cross_chunk_links::FinalEsmInitMetadataAvailability,
  order_wrap_state::OrderWrapState,
};

/// Whether only `import()` reaches the entry of `chunk`.
pub(super) fn is_pure_dynamic_entry_chunk(chunk: &Chunk) -> bool {
  matches!(chunk.kind, ChunkKind::EntryPoint { meta, .. } if meta == ChunkMeta::DynamicImported)
}

impl GenerateStage<'_> {
  /// Whether code can see a key that the chunk `chunk_idx` adds to the namespace of the dynamic
  /// entry `module`. `preserveEntrySignatures` does not apply to a pure dynamic entry, as in
  /// Rollup. The function returns `false` in each of these conditions:
  /// - The entry module is CommonJS. In ESM output, `import()` of it gives `__toESM(m.default)`.
  ///   In CJS output, it gives `__toESM(require(...).default)`, or `__toESM(require(...))` when
  ///   the chunk exports only a default. So the other exports of the chunk are not keys of the
  ///   result. An extraction would also be wrong: it reads `n.require_xxx()`, and in CJS output
  ///   the chunk exports only the `default` value of the entry (`issues/11397_cjs_entry_target`).
  /// - All known uses of the namespace read exports of the module
  ///   (`dynamic_entry_partial_usage_allows_plain_merge`).
  /// - The importers already read an extracted namespace of the module from `chunk_idx`.
  /// - Each `import()` and `import.defer()` of the module reads a binding that the chunk exports
  ///   for `import defer` (`dynamic_entry_reads_defer_bindings`).
  pub(super) fn dynamic_entry_namespace_is_observable(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    module: ModuleIdx,
  ) -> bool {
    !self.link_output.module_table[module]
      .as_normal()
      .is_some_and(|entry| entry.exports_kind.is_commonjs())
      && !self.dynamic_entry_partial_usage_allows_plain_merge(module)
      && !chunk_graph
        .common_chunk_exported_facade_chunk_namespace
        .get(&chunk_idx)
        .is_some_and(|modules| modules.contains(&module))
      && !self.dynamic_entry_reads_defer_bindings(chunk_graph, chunk_idx, module)
  }

  /// Whether each `import()` and `import.defer()` of the dynamic entry `module` reads a binding
  /// that the chunk `chunk_idx` exports for `import defer`, not the namespace of the chunk
  /// (`rewrite_dynamic_import_of_defer_target`). The chunk must hold the module. The module must
  /// be an `import.defer()` target, or the chunk must be its lazy host
  /// (`ChunkGraph::lazy_dynamic_defer_host`). `import()` reads `init_x` and `x_exports`, or only
  /// `x_exports` for a module with no wrapper (a JSON module, when `strictExecutionOrder` does not
  /// wrap it). `import.defer()` reads `x_deferred`.
  fn dynamic_entry_reads_defer_bindings(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    module: ModuleIdx,
  ) -> bool {
    let meta = &self.link_output.metas[module];
    chunk_graph.module_to_chunk[module] == Some(chunk_idx)
      && (meta.dynamic_defer_target || chunk_graph.lazy_dynamic_defer_host(module, meta).is_some())
  }

  /// Why the importers of the dynamic entry `module` in the chunk `chunk_idx` cannot read an
  /// extracted namespace. The `import()` of such an entry can still give extra keys, as before.
  /// The decision and the debug assertion call this function before and after
  /// `find_entry_level_external_module`, so each reason must be the same in both phases.
  pub(super) fn dynamic_entry_extraction_skip_reason(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    module: ModuleIdx,
  ) -> Option<&'static str> {
    if !self.dynamic_entry_supports_namespace_extraction(module) {
      return Some("the entry exports `then` or has dynamic exports");
    }
    // `find_entry_level_external_module` (in `finalize_chunk_plan`) moves each `export *` of an
    // external module that the entry reaches through star chains to the chunk, and it clears
    // `has_dynamic_exports` of the barrels on the chain. Before that step, this list is empty and
    // the check above gives the reason. After it, this list gives the reason
    // (`issues/11397_external_star_barrel`). A module-local namespace does not reproduce the
    // chunk-level `export *`, as the same check in `order_wrapping.rs` says.
    if !chunk_graph.chunk_table[chunk_idx].entry_level_external_module_idx.is_empty() {
      return Some("the entry re-exports an external module through `export *`");
    }
    // The chunk export `var value = import_a.value;` reads a CommonJS property one time, when the
    // chunk runs. The getter of an extracted namespace reads the property each time, so a later
    // write to the CommonJS module changes the value (`issues/11397_cjs_reexport_timing`).
    //
    // In CJS output, an external binding is a property read too (`node_path.sep`). Its chunk export
    // is already a getter, so the extracted namespace reads it the same way.
    let symbol_db = &self.link_output.symbol_db;
    let module_table = &self.link_output.module_table;
    let reexports_commonjs_property =
      self.link_output.metas[module].canonical_exports(false).any(|(_, export)| {
        symbol_db
          .get(symbol_db.canonical_ref_for(export.symbol_ref))
          .namespace_alias
          .as_ref()
          .is_some_and(|alias| !module_table[alias.namespace_ref.owner].is_external())
      });
    reexports_commonjs_property.then_some("the entry re-exports a CommonJS property")
  }

  /// Finds the pure dynamic entry chunks that export bindings that their entry module does not
  /// export. `candidates` holds the chunks that a merge gave modules of other entries. The function
  /// adds the chunks that own a binding that a live pure dynamic entry re-exports, since such a
  /// chunk exports the binding to the entry's chunk. The check runs the link pass once, read-only,
  /// on the current layout. Its symbol ownership stays in the pass-local table, so the symbol
  /// database does not change.
  ///
  /// The caller extracts the namespace of each returned entry on its own chunk. The chunk stays an
  /// entry chunk, so the order decisions of later steps do not change.
  pub(super) fn find_leaking_dynamic_entry_namespaces(
    &mut self,
    chunk_graph: &ChunkGraph,
    candidates: &mut FxHashSet<ChunkIdx>,
    used_symbol_refs_builder: &UsedSymbolRefsBuilder,
  ) -> Vec<(ChunkIdx, ModuleIdx)> {
    let symbol_db = &self.link_output.symbol_db;
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      let ChunkKind::EntryPoint { meta, module, .. } = chunk.kind else { continue };
      if meta != ChunkMeta::DynamicImported || !chunk_graph.chunk_is_live(chunk_idx) {
        continue;
      }
      for export in self.link_output.metas[module].resolved_exports.values() {
        let owner = symbol_db.canonical_ref_resolving_namespace(export.symbol_ref).owner;
        if !export.came_from_commonjs
          && let Some(owner_chunk) = chunk_graph.module_to_chunk[owner]
          && owner_chunk != chunk_idx
        {
          candidates.insert(owner_chunk);
        }
      }
    }
    if candidates.is_empty()
      || self.options.preserve_modules
      || self.options.code_splitting.is_disabled()
    {
      return vec![];
    }
    let mut leaks: Vec<(ChunkIdx, ModuleIdx)> = candidates
      .iter()
      .filter_map(|&chunk_idx| {
        let ChunkKind::EntryPoint { meta, module, .. } = chunk_graph.chunk_table[chunk_idx].kind
        else {
          return None;
        };
        (meta == ChunkMeta::DynamicImported
          && chunk_graph.module_to_chunk[module] == Some(chunk_idx)
          && self.dynamic_entry_namespace_is_observable(chunk_graph, chunk_idx, module)
          && self.dynamic_entry_extraction_skip_reason(chunk_graph, chunk_idx, module).is_none())
        .then_some((chunk_idx, module))
      })
      .collect();
    if leaks.is_empty() {
      return leaks;
    }
    leaks.sort_unstable();
    // The link pass keeps a namespace object only when its module has `namespace_included`, and
    // `finalize_chunk_plan` makes that decision later. Without it, the check cannot see a chunk
    // that exports a namespace object to another chunk (`import * as route from './route.mjs'`).
    // Make the decision for this layout, and restore the old flags after the check, so the later
    // steps see the same state as before. `finalized_module_namespace_ref_usage` writes only these
    // flags, and the link pass takes `&self`.
    let namespace_included: Vec<bool> =
      self.link_output.metas.iter().map(|meta| meta.namespace_included).collect();
    self.finalized_module_namespace_ref_usage(chunk_graph, &OrderWrapState::default());
    let state = self.compute_cross_chunk_link_state(
      chunk_graph,
      used_symbol_refs_builder.view(),
      &OrderWrapState::default(),
      FinalEsmInitMetadataAvailability::Unavailable,
    );
    leaks.retain(|&(chunk_idx, module)| {
      self.chunk_exports_internal_symbols(
        chunk_idx,
        module,
        used_symbol_refs_builder.view(),
        &state,
      )
    });
    // The state is stale after the flag restore.
    drop(state);
    for (meta, flag) in self.link_output.metas.iter_mut().zip(namespace_included) {
      meta.namespace_included = flag;
    }
    leaks
  }

  /// Whether the runtime can go into `target_chunk_idx` when that chunk is a pure dynamic entry
  /// chunk. When a consumer other than the host uses helpers, the host exports them, and the
  /// namespace of `import()` gets the helper keys. This is harmless only when no code can see the
  /// keys, or when the importers read an extracted namespace (`issues/11397_runtime`).
  ///
  /// An entry with a reason in `dynamic_entry_extraction_skip_reason` keeps the host, as before.
  /// Its chunk keeps the other extra keys, so a separate runtime file would not remove the leak.
  pub(super) fn runtime_merge_preserves_dynamic_entry_namespace(
    &self,
    chunk_graph: &ChunkGraph,
    target_chunk_idx: ChunkIdx,
    consumer_chunks: &FxHashSet<ChunkIdx>,
  ) -> bool {
    if consumer_chunks.iter().all(|&chunk_idx| chunk_idx == target_chunk_idx) {
      return true;
    }
    let ChunkKind::EntryPoint { meta, module, .. } = chunk_graph.chunk_table[target_chunk_idx].kind
    else {
      return true;
    };
    meta != ChunkMeta::DynamicImported
      || !self.dynamic_entry_namespace_is_observable(chunk_graph, target_chunk_idx, module)
      || self.dynamic_entry_extraction_skip_reason(chunk_graph, target_chunk_idx, module).is_some()
  }

  /// Asserts the rule of `find_leaking_dynamic_entry_namespaces` on the final links: a pure dynamic
  /// entry chunk whose namespace code can see exports only public exports of its entry module. The
  /// check runs before runtime placement and order lowering, so a gap in it
  /// shows up here in the test suite. A release build keeps the extra keys in such a case.
  #[cfg(debug_assertions)]
  pub(super) fn debug_assert_dynamic_entry_namespaces(
    &self,
    chunk_graph: &ChunkGraph,
    used: UsedSymbolRefsView<'_>,
    state: &CrossChunkLinkState,
  ) {
    if self.options.preserve_modules || self.options.code_splitting.is_disabled() {
      return;
    }
    let dropped_export_owners = self.dropped_entry_export_owners(chunk_graph);
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      let ChunkKind::EntryPoint { meta, module, .. } = chunk.kind else { continue };
      if meta != ChunkMeta::DynamicImported
        || chunk.modules.is_empty()
        || !chunk_graph.chunk_is_live(chunk_idx)
        || !self.dynamic_entry_namespace_is_observable(chunk_graph, chunk_idx, module)
      {
        continue;
      }
      // These entries can keep extra keys. The reason names the open gap.
      if self.dynamic_entry_extraction_skip_reason(chunk_graph, chunk_idx, module).is_some() {
        continue;
      }
      // The check covers only a chunk that holds its entry module. A group with
      // `includeDependenciesRecursively: false` can take the entry module and leave its
      // dependencies in this chunk, which then exports them to the group chunk (`issues/11155`).
      if chunk_graph.module_to_chunk[module] != Some(chunk_idx) {
        continue;
      }
      // The check does not cover this layout either. See `dropped_entry_export_owners`.
      if chunk.modules.iter().any(|module| dropped_export_owners.contains(module)) {
        continue;
      }
      debug_assert!(
        !self.chunk_exports_internal_symbols(chunk_idx, module, used, state),
        "the chunk {chunk_idx:?} of the dynamic entry `{}` exports bindings that the entry module \
         does not export, so `import()` gives a namespace with extra keys",
        self.link_output.module_table[module].stable_id(),
      );
    }
  }

  /// The modules that own the bindings of the exports that an entry drops with
  /// `preserveEntrySignatures: false`. The link pass still imports these bindings into the entry
  /// chunk (`register_entry_export_depended_symbols`), so the chunk that holds such a module
  /// exports them, also when it is the chunk of a dynamic entry
  /// (`optimization/chunk_merging/already_loaded_entry_reexport_service_edge`).
  #[cfg(debug_assertions)]
  fn dropped_entry_export_owners(&self, chunk_graph: &ChunkGraph) -> FxHashSet<ModuleIdx> {
    let symbol_db = &self.link_output.symbol_db;
    chunk_graph
      .chunk_table
      .iter()
      .filter_map(|chunk| match chunk.kind {
        // The same condition as `needs_export_entry_signatures` in `compute_cross_chunk_links`.
        ChunkKind::EntryPoint { meta, module, .. }
          if !meta.contains(ChunkMeta::DynamicImported)
            && matches!(
              normalize_preserve_entry_signature(
                &self.link_output.overrode_preserve_entry_signature_map,
                self.options,
                module,
              ),
              PreserveEntrySignatures::False
            ) =>
        {
          Some(module)
        }
        _ => None,
      })
      // Service imports for dropped signatures only come from entries without a CJS wrapper.
      .filter(|&module| !matches!(self.link_output.metas[module].wrap_kind(), WrapKind::Cjs))
      .flat_map(|module| self.link_output.metas[module].resolved_exports.values())
      .filter(|export| !export.came_from_commonjs)
      .map(|export| symbol_db.canonical_ref_resolving_namespace(export.symbol_ref).owner)
      .collect()
  }
}
