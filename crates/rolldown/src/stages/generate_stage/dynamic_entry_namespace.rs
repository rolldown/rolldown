//! The namespace that `import()` gives for a pure dynamic entry chunk.
//!
//! See internal-docs/code-splitting/implementation.md#dynamic-entry-namespaces.

#[cfg(debug_assertions)]
use rolldown_common::UsedSymbolRefsView;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ModuleIdx, PreserveEntrySignatures, UsedSymbolRefsBuilder,
};
use rustc_hash::FxHashSet;

use crate::chunk_graph::ChunkGraph;

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
  /// entry `module`. The function returns `false` in each of these conditions:
  /// - The global `preserveEntrySignatures` is `allow-extension` or `false`.
  /// - The entry module is CommonJS. `import()` of it gives `__toESM(m.default)` in all formats,
  ///   so the other exports of the chunk are not keys of the result. An extraction would also be
  ///   wrong: it reads `n.require_xxx()`, and in CJS output the chunk exports only the `default`
  ///   value of the entry (`issues/11397_cjs_entry_target`).
  /// - All known uses of the namespace read exports of the module
  ///   (`dynamic_entry_partial_usage_allows_plain_merge`).
  /// - The importers already read an extracted namespace of the module from `chunk_idx`.
  pub(super) fn dynamic_entry_namespace_is_observable(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    module: ModuleIdx,
  ) -> bool {
    matches!(
      self.options.preserve_entry_signatures,
      PreserveEntrySignatures::Strict | PreserveEntrySignatures::ExportsOnly
    ) && !self.link_output.module_table[module]
      .as_normal()
      .is_some_and(|entry| entry.exports_kind.is_commonjs())
      && !self.dynamic_entry_partial_usage_allows_plain_merge(module)
      && !chunk_graph
        .common_chunk_exported_facade_chunk_namespace
        .get(&chunk_idx)
        .is_some_and(|modules| modules.contains(&module))
  }

  /// Why the importers of the dynamic entry `module` in the chunk `chunk_idx` cannot read an
  /// extracted namespace. Such an entry keeps the extra keys of its chunk namespace, as before.
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
  /// export. `candidates` are the chunks that a merge gave modules of other entries. The check
  /// runs the link pass once, read-only, on the current layout. Its symbol ownership stays in the
  /// pass-local table, so the symbol database does not change.
  ///
  /// The caller extracts the namespace of each returned entry on its own chunk. The chunk stays an
  /// entry chunk, so the order decisions of later steps do not change.
  pub(super) fn find_leaking_dynamic_entry_namespaces(
    &mut self,
    chunk_graph: &ChunkGraph,
    candidates: &FxHashSet<ChunkIdx>,
    used_symbol_refs_builder: &UsedSymbolRefsBuilder,
  ) -> Vec<(ChunkIdx, ModuleIdx)> {
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
    // Do not assign the chunk orders here. The check needs only the sets of exported symbols, and
    // `rebuild_sorted_chunk_idx_vec` also sorts the chunks that facade elimination removes later.
    // Such a chunk would keep an order from this call and take a chunk name before a live chunk.
    //
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
  /// check runs before facade elimination, runtime placement and order lowering, so a gap in it
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
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      let ChunkKind::EntryPoint { meta, module, .. } = chunk.kind else { continue };
      if meta != ChunkMeta::DynamicImported
        || chunk.modules.is_empty()
        || !chunk_graph.chunk_is_live(chunk_idx)
        || !self.dynamic_entry_namespace_is_observable(chunk_graph, chunk_idx, module)
      {
        continue;
      }
      // These entries keep the extra keys. The reason names the open gap.
      if self.dynamic_entry_extraction_skip_reason(chunk_graph, chunk_idx, module).is_some() {
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
}
