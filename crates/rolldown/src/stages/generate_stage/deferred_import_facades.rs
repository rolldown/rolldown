use super::{GenerateStage, order_wrap_state::OrderWrapState};
use crate::chunk_graph::ChunkGraph;
use oxc_str::format_compact_str;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, CrossChunkImportItem, Module, OutputExports, OutputFormat,
  StaticImport, SymbolRef,
};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::FxHashSet;
use std::collections::BTreeMap;

impl GenerateStage<'_> {
  /// Native import() resolves its promise with the file namespace before our finalizer can
  /// select the deferred namespace. A public `then` on that file would intercept the load.
  /// Re-export just the service bindings through one ordinary ESM chunk with safe names.
  /// See internal-docs/import-defer/implementation.md.
  pub(super) fn add_deferred_import_facades(&self, graph: &mut ChunkGraph, order: &OrderWrapState) {
    if graph.chunk_table.len() < 2 || !matches!(self.options.format, OutputFormat::Esm) {
      return;
    }
    let unsafe_hosts: FxHashSet<_> = graph
      .chunk_table
      .iter_enumerated()
      .filter_map(|(idx, chunk)| {
        (!chunk.entry_level_external_module_idx.is_empty()
          || chunk.exports_to_other_chunks.values().flatten().any(|name| name == "then"))
        .then_some(idx)
      })
      .collect();
    if unsafe_hosts.is_empty() {
      return;
    }
    let mut symbols: BTreeMap<ChunkIdx, FxHashSet<SymbolRef>> = BTreeMap::new();
    let mut importers = FxHashSet::default();
    for module in self.link_output.module_table.iter().filter_map(Module::as_normal) {
      let Some(from) = graph.module_to_chunk[module.idx] else { continue };
      for rec in &module.import_records {
        let Some(target) = rec.resolved_module else { continue };
        if !self.is_live_dynamic_import(module.idx, rec, target) {
          continue;
        }
        let Some(host) = graph.module_to_chunk[target] else { continue };
        let meta = &self.link_output.metas[target];
        if from == host
          || !unsafe_hosts.contains(&host)
          || !graph.dynamic_import_reads_defer_bindings(target, meta, rec.is_defer_phase())
        {
          continue;
        }
        importers.insert(from);
        let refs = symbols.entry(host).or_default();
        for symbol in [
          self.link_output.module_table[target].namespace_object_ref(),
          meta.deferred_namespace_ref,
          order.esm_init_target(target, meta).map(|t| t.wrapper_ref),
        ]
        .into_iter()
        .flatten()
        {
          if graph.chunk_table[host].exports_to_other_chunks.contains_key(&symbol) {
            refs.insert(symbol);
          }
        }
      }
    }
    for (host, refs) in symbols {
      let source = &graph.chunk_table[host];
      let mut facade = Chunk::new(
        Some("defer-import".into()),
        None,
        source.bits.clone(),
        vec![],
        ChunkKind::Common,
        source.input_base.clone(),
        None,
      );
      facade.exec_order = source.exec_order;
      facade.output_exports = OutputExports::Named;
      facade.static_imports.push(StaticImport::Chunk(host));
      facade.cross_chunk_imports.push(host);
      // Use the deterministic export names of the host to order service bindings.
      let mut refs = refs.into_iter().collect::<Vec<_>>();
      refs.sort_by_key(|symbol| &source.exports_to_other_chunks[symbol][0]);
      for (ordinal, symbol) in refs.iter().enumerate() {
        facade.exports_to_other_chunks.insert(*symbol, vec![format_compact_str!("s{ordinal}")]);
      }
      facade.imports_from_other_chunks.insert(
        host,
        refs.into_iter().map(|import_ref| CrossChunkImportItem { import_ref }).collect(),
      );
      let idx = graph.add_chunk(facade);
      graph.sorted_chunk_idx_vec.push(idx);
      graph.deferred_import_facades.insert(host, idx);
    }
    // Recollect in the same module and record order as the original links. An ordinary
    // import of a public entry can still name the host directly.
    for importer in importers {
      let mut imports = FxIndexSet::default();
      for &idx in &graph.chunk_table[importer].modules {
        let Some(module) = self.link_output.module_table[idx].as_normal() else { continue };
        for record in &module.import_records {
          if let Some(target) = record.resolved_module
            && self.link_output.module_table[target].as_normal().is_some()
          {
            self.collect_dynamic_chunk_import(graph, idx, record, target, &mut imports);
          }
        }
      }
      graph.chunk_table[importer].cross_chunk_dynamic_imports = imports.into_iter().collect();
    }
  }
}
