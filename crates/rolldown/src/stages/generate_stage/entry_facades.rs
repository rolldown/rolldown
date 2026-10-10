use super::{
  GenerateStage,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  compute_cross_chunk_links::CrossChunkLinkState,
};
use crate::chunk_graph::ChunkGraph;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ExportsKind, OutputFormat, PreserveEntrySignatures,
  UsedSymbolRefsView,
};
use rustc_hash::FxHashSet;

impl GenerateStage<'_> {
  pub(super) fn preserve_strict_entry_signatures(
    &self,
    chunk_graph: &mut ChunkGraph,
    used: UsedSymbolRefsView<'_>,
    state: &CrossChunkLinkState,
  ) -> bool {
    if self.options.preserve_modules || self.options.code_splitting.is_disabled() {
      return false;
    }
    let candidates: Vec<_> = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter_map(|(chunk_idx, chunk)| {
        let ChunkKind::EntryPoint { module, meta, .. } = chunk.kind else { return None };
        if chunk.modules.is_empty()
          || !meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk)
          || !matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::Strict))
        {
          return None;
        }
        let exported_symbols = &state.index_chunk_exported_symbols[chunk_idx];
        // Predefined names come only from this entry's public exports.
        if exported_symbols.values().all(|names| !names.is_empty()) {
          return None;
        }
        let entry_meta = &self.link_output.metas[module];
        let public_symbols: FxHashSet<_> = entry_meta
          .canonical_exports(false)
          .map(|(_, export)| self.link_output.symbol_db.canonical_ref_for(export.symbol_ref))
          .collect();
        let entry = self.link_output.module_table[module].as_normal().unwrap();
        let extra = exported_symbols.keys().any(|symbol_ref| {
          let canonical = self.link_output.symbol_db.canonical_ref_for(*symbol_ref);
          if public_symbols.contains(&canonical) {
            return false;
          }
          // A CJS entry's own symbols are served by its public module.exports value.
          if !matches!(self.options.format, OutputFormat::Esm)
            && !matches!(entry.exports_kind, ExportsKind::Esm)
            && canonical.owner == module
          {
            return false;
          }
          self.cross_chunk_symbol_is_live(used, &state.order_live_symbols, canonical)
        });
        extra.then_some(chunk_idx)
      })
      .collect();
    let changed = !candidates.is_empty();
    for chunk_idx in candidates {
      self.create_entry_facade(chunk_graph, chunk_idx);
    }
    changed
  }

  pub(super) fn create_entry_facade(
    &self,
    chunk_graph: &mut ChunkGraph,
    chunk_idx: ChunkIdx,
  ) -> bool {
    let chunk = &mut chunk_graph.chunk_table[chunk_idx];
    let ChunkKind::EntryPoint { module, meta, .. } = chunk.kind else { return false };
    if chunk.modules.is_empty() {
      return false;
    }
    let mut facade = Chunk::new(
      chunk.name.take(),
      chunk.file_name.take(),
      chunk.bits.clone(),
      vec![],
      std::mem::replace(&mut chunk.kind, ChunkKind::Common),
      chunk.input_base.clone(),
      chunk.preserve_entry_signature.take(),
    );
    chunk.add_creation_reason(
      ChunkCreationReason::CommonChunk { bits: &facade.bits, link_output: self.link_output },
      self.options,
    );
    facade.add_creation_reason(
      ChunkCreationReason::Entry {
        is_user_defined_entry: meta.contains(ChunkMeta::UserDefinedEntry),
        entry_module_id: self.link_output.module_table[module].stable_id(),
        name: self
          .link_output
          .entries
          .get(&module)
          .and_then(|entries| entries.first())
          .and_then(|entry| entry.name.as_ref()),
      },
      self.options,
    );
    let facade_idx = chunk_graph.add_chunk(facade);
    if chunk_graph.module_to_chunk[module] == Some(chunk_idx)
      && self.link_output.module_table[module].as_normal().unwrap().exports_kind.is_commonjs()
      && self.link_output.metas[module].wrap_kind().is_none()
    {
      // An unwrapped CJS entry writes directly to this file's module.exports.
      chunk_graph.chunk_table[chunk_idx].modules.retain(|module_idx| *module_idx != module);
      chunk_graph.add_module_to_chunk(
        module,
        facade_idx,
        self.link_output.metas[module].depended_runtime_helper,
      );
    }
    if chunk_graph.entry_module_to_entry_chunk.get(&module) == Some(&chunk_idx) {
      chunk_graph.entry_module_to_entry_chunk.insert(module, facade_idx);
    }
    if let Some(reference_ids) = chunk_graph.chunk_idx_to_reference_ids.remove(&chunk_idx) {
      chunk_graph.chunk_idx_to_reference_ids.insert(facade_idx, reference_ids);
    }
    true
  }
}
