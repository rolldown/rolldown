use itertools::Itertools;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkReasonType, ConcatenateWrappedModuleKind, ImportKind,
  ImportRecordMeta, ModuleIdx, RuntimeHelper, StmtInfoMeta, SymbolOrMemberExprRef, WrapKind,
};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::FxHashSet;

use super::{LOG_TARGET, place::compute_placement};
use crate::{
  chunk_graph::ChunkGraph,
  stages::generate_stage::{
    GenerateStage, compute_cross_chunk_links::CrossChunkLinkState, order_wrap_state::OrderWrapState,
  },
};

/// Chunk-level facts every candidate is checked against.
struct SelectionFacts<'a> {
  link_state: &'a CrossChunkLinkState,
  runtime_chunk: ChunkIdx,
  /// Chunks some `import()` resolves to, plus chunks hosting a collapsed dynamic-entry facade.
  dynamic_targets: &'a FxHashSet<ChunkIdx>,
  /// Chunks owning a symbol that another chunk re-exports with `export { x }`: an ESM export
  /// needs a local binding, which a bridge property read cannot provide.
  foreign_exported_owner_chunks: &'a FxHashSet<ChunkIdx>,
  /// Chunks whose symbols a module with direct `eval` references by name.
  eval_read_chunks: &'a FxHashSet<ChunkIdx>,
  max_size: usize,
}

impl GenerateStage<'_> {
  /// The selection point. `link_state` is the final cross-chunk derivation: order lowering and
  /// the unused-runtime sweep are done, liveness is sealed, and nothing after this moves a module
  /// between chunks. Returns whether any record was selected; the caller then derives the links
  /// again so the registry demand registered here reaches the runtime chunk's imports and exports.
  pub(in crate::stages::generate_stage) fn select_inline_common_chunks(
    &mut self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    order_state: &mut OrderWrapState,
  ) -> bool {
    let Some(max_size) = self.options.inline_common_chunks.as_ref().map(|options| options.max_size)
    else {
      return false;
    };

    // The registry lives in the runtime module. `try_merge_runtime_chunk` never merges the
    // runtime while the option is on, and any candidate's members demand a helper (`__esmMin` for
    // an order-wrapped module, `__commonJS` for a CommonJS one), so the sweep keeps the runtime
    // whenever there is something to select. A build that still has no standalone runtime chunk
    // has no candidate either.
    let runtime_idx = self.link_output.runtime.id();
    let Some(runtime_chunk) = chunk_graph.module_to_chunk[runtime_idx].filter(|chunk_idx| {
      chunk_graph.chunk_is_live(*chunk_idx)
        && chunk_graph.chunk_table[*chunk_idx].modules.len() == 1
    }) else {
      tracing::debug!(target: LOG_TARGET, "no standalone runtime chunk, nothing selected");
      return false;
    };

    let is_live = |chunk_idx: ChunkIdx| {
      !chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
    };
    // The edges `commit_cross_chunk_links` will write; the projection step recomputes the
    // placement from the committed table and asserts it is the same. Dead chunks and self-edges
    // are skipped where the table is read.
    let static_importees = link_state.static_importee_table(chunk_graph);
    let dynamic_targets = link_state
      .index_cross_chunk_dynamic_imports
      .iter()
      .flatten()
      .copied()
      .chain(chunk_graph.common_chunk_exported_facade_chunk_namespace.keys().copied())
      .chain(chunk_graph.entry_module_to_entry_chunk.values().copied())
      .collect::<FxHashSet<_>>();
    let foreign_exported_owner_chunks = self.foreign_exported_owner_chunks(chunk_graph, link_state);
    let eval_read_chunks = self.eval_read_chunks(chunk_graph);
    let facts = SelectionFacts {
      link_state,
      runtime_chunk,
      dynamic_targets: &dynamic_targets,
      foreign_exported_owner_chunks: &foreign_exported_owner_chunks,
      eval_read_chunks: &eval_read_chunks,
      max_size,
    };

    let mut candidates = FxIndexSet::default();
    for chunk_idx in chunk_graph.sorted_chunk_idx_vec.iter().copied() {
      if !is_live(chunk_idx) {
        continue;
      }
      let chunk = &chunk_graph.chunk_table[chunk_idx];
      match self.keep_as_file_reason(chunk_idx, chunk, chunk_graph, order_state, &facts) {
        Some(reason) => {
          tracing::debug!(
            target: LOG_TARGET,
            chunk = chunk_idx.raw(),
            modules = ?self.module_ids_of(chunk),
            reason,
            "kept as file"
          );
        }
        None => {
          candidates.insert(chunk_idx);
        }
      }
    }

    // A carrier prints a record's imports as its own. A file that is both a reader of the record
    // (so it carries it) and one of the record's importees would have to import itself, so a
    // static-import cycle that mixes records and files keeps every record in it as a file.
    // Record-only cycles are fine: records reach each other through bridges, not imports.
    let mut graph = petgraph::prelude::DiGraphMap::<ChunkIdx, ()>::new();
    for (chunk_idx, importees) in static_importees.iter_enumerated() {
      if !is_live(chunk_idx) || chunk_idx == runtime_chunk {
        continue;
      }
      graph.add_node(chunk_idx);
      for importee in importees {
        if *importee != runtime_chunk && is_live(*importee) {
          graph.add_edge(chunk_idx, *importee, ());
        }
      }
    }
    for component in petgraph::algo::tarjan_scc(&graph) {
      let selected =
        component.iter().copied().filter(|chunk_idx| candidates.contains(chunk_idx)).collect_vec();
      if selected.is_empty() || selected.len() == component.len() {
        continue;
      }
      for chunk_idx in selected {
        candidates.shift_remove(&chunk_idx);
        tracing::debug!(
          target: LOG_TARGET,
          chunk = chunk_idx.raw(),
          modules = ?self.module_ids_of(&chunk_graph.chunk_table[chunk_idx]),
          reason = "in a static import cycle with a file",
          "kept as file"
        );
      }
    }

    let mut records = candidates;
    // A carrier names its own modules and its records' in one pass, and a module with direct
    // `eval` reads its bindings by their source names, which the renamer does not protect; a
    // record such a file would print stays a file. Removing a record moves other records'
    // carriers, so the placement is recomputed until every record passes.
    let eval_files = self.eval_files(chunk_graph);
    let placement = loop {
      let placement = compute_placement(
        &static_importees,
        &link_state.index_cross_chunk_dynamic_imports,
        is_live,
        |chunk_idx| {
          chunk_graph.chunk_table[chunk_idx].is_user_defined_entry()
            || chunk_graph.chunk_idx_to_reference_ids.contains_key(&chunk_idx)
        },
        |chunk_idx| chunk_graph.chunk_table[chunk_idx].exec_order,
        &records,
      );
      let rejected = records
        .iter()
        .copied()
        .filter_map(|record| {
          if !placement.is_carried(record) {
            return Some((record, "no file reads it"));
          }
          let eval_carrier = placement
            .carried
            .iter()
            .any(|(file, carried)| eval_files.contains(file) && carried.contains(&record));
          eval_carrier.then_some((record, "a file that prints it holds a module with direct eval"))
        })
        .collect_vec();
      if rejected.is_empty() {
        break placement;
      }
      for (record, reason) in rejected {
        records.shift_remove(&record);
        tracing::debug!(
          target: LOG_TARGET,
          chunk = record.raw(),
          modules = ?self.module_ids_of(&chunk_graph.chunk_table[record]),
          reason,
          "kept as file"
        );
      }
    };
    if records.is_empty() {
      tracing::debug!(target: LOG_TARGET, "no common chunk selected");
      return false;
    }
    for record in &records {
      let chunk = &chunk_graph.chunk_table[*record];
      tracing::debug!(
        target: LOG_TARGET,
        chunk = record.raw(),
        modules = ?self.module_ids_of(chunk),
        size = self.chunk_size(chunk),
        readers = ?placement.readers.iter().filter(|(_, read)| read.contains(record)).map(|(reader, _)| reader.raw()).sorted_unstable().collect::<Vec<_>>(),
        "selected as record"
      );
    }

    // Every file that reads a record imports the registry helpers from the runtime chunk. The
    // demand goes through the synthetic-statement channel order wrappers use, so the next link
    // derivation imports and exports the helpers like any other runtime symbol.
    for file in placement.reading_files.iter().copied() {
      let mut helpers = RuntimeHelper::ShareRequire;
      if placement.carried.contains_key(&file) {
        helpers |= RuntimeHelper::Share | RuntimeHelper::ShareExport;
      }
      order_state.insert_runtime_helper_demand(file, helpers);
    }
    order_state.compute_runtime_symbol_closure(
      &self.link_output.runtime,
      &self.link_output.stmt_infos[runtime_idx],
      &self.link_output.symbol_db,
    );

    self.inline_state.set_selection(records, placement, runtime_chunk);
    true
  }

  fn module_ids_of(&self, chunk: &Chunk) -> Vec<String> {
    chunk
      .modules
      .iter()
      .map(|module_idx| self.link_output.module_table[*module_idx].stable_id().to_string())
      .collect()
  }

  fn chunk_size(&self, chunk: &Chunk) -> usize {
    chunk.modules.iter().map(|module_idx| self.link_output.module_table[*module_idx].size()).sum()
  }

  /// `None` means the chunk is a candidate. The first failing rule names why it stays a file.
  fn keep_as_file_reason(
    &self,
    chunk_idx: ChunkIdx,
    chunk: &Chunk,
    chunk_graph: &ChunkGraph,
    order_state: &OrderWrapState,
    facts: &SelectionFacts<'_>,
  ) -> Option<&'static str> {
    if !matches!(chunk.kind, ChunkKind::Common) {
      return Some("entry chunk");
    }
    if !matches!(*chunk.chunk_reason_type, ChunkReasonType::Common) {
      return Some("not created by common code splitting");
    }
    if chunk_idx == facts.runtime_chunk {
      return Some("runtime chunk");
    }
    if chunk_graph.chunk_idx_to_reference_ids.contains_key(&chunk_idx) {
      return Some("emitted chunk");
    }
    if facts.dynamic_targets.contains(&chunk_idx) {
      return Some("target of a dynamic import");
    }
    if chunk.modules.is_empty() {
      return Some("empty chunk");
    }
    if facts.foreign_exported_owner_chunks.contains(&chunk_idx) {
      return Some("another chunk re-exports one of its symbols");
    }
    if facts.eval_read_chunks.contains(&chunk_idx) {
      return Some("a module with direct eval references its symbols");
    }
    if !facts.link_state.index_chunk_direct_imports_from_external_modules[chunk_idx].is_empty()
      || !facts.link_state.index_chunk_indirect_imports_from_external_modules[chunk_idx].is_empty()
      || !facts.link_state.index_chunk_dynamic_imports_from_external_modules[chunk_idx].is_empty()
    {
      return Some("imports an external module");
    }
    for module_idx in chunk.modules.iter().copied() {
      if let Some(reason) = self.inline_common_chunk_module_bailout(module_idx) {
        return Some(reason);
      }
      let meta = &self.link_output.metas[module_idx];
      if order_state.esm_init_target(module_idx, meta).is_none()
        && !matches!(meta.wrap_kind(), WrapKind::Cjs)
      {
        return Some("member has no wrapper");
      }
    }
    if facts.max_size != usize::MAX && self.chunk_size(chunk) >= facts.max_size {
      return Some("not smaller than maxSize");
    }
    None
  }

  pub(in crate::stages::generate_stage) fn prefers_inline_common_chunk(
    &self,
    modules: &[ModuleIdx],
  ) -> bool {
    let Some(options) = &self.options.inline_common_chunks else {
      return false;
    };
    let mut size = 0;
    let mut has_module = false;
    for &module_idx in modules {
      if module_idx == self.link_output.runtime.id() {
        continue;
      }
      has_module = true;
      if self.inline_common_chunk_module_bailout(module_idx).is_some() {
        return false;
      }
      size += self.link_output.module_table[module_idx].size();
    }
    has_module && (options.max_size == usize::MAX || size < options.max_size)
  }

  fn inline_common_chunk_module_bailout(&self, module_idx: ModuleIdx) -> Option<&'static str> {
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return Some("member is not a normal module");
    };
    if self.link_output.entries.contains_key(&module_idx) {
      return Some("hosts an entry module");
    }
    if self.inline_state.is_excluded_module(module_idx) {
      return Some("member matches `exclude`");
    }
    let meta = &self.link_output.metas[module_idx];
    if meta.is_tla_or_contains_tla_dependency {
      return Some("member uses top-level await");
    }
    if module.meta.has_eval() {
      return Some("member uses direct eval");
    }
    if !matches!(meta.concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::None) {
      return Some("member is a concatenated wrapped module");
    }
    for (stmt_info_idx, stmt_info) in self.link_output.stmt_infos[module_idx].iter_enumerated() {
      if !meta.stmt_info_included.has_bit(stmt_info_idx) {
        continue;
      }
      if stmt_info.meta.contains(StmtInfoMeta::ImportMeta) {
        return Some("member uses import.meta");
      }
      if stmt_info.meta.contains(StmtInfoMeta::NonStaticDynamicImport) {
        return Some("member uses dynamic import");
      }
      for rec_idx in &stmt_info.import_records {
        let rec = &module.import_records[*rec_idx];
        match rec.kind {
          ImportKind::Import | ImportKind::Require => match rec.resolved_module {
            Some(importee_idx) => {
              if self.link_output.module_table[importee_idx].is_external() {
                return Some("member imports an external module");
              }
            }
            None => return Some("member has an unresolved import"),
          },
          ImportKind::DynamicImport => {
            if !rec.meta.contains(ImportRecordMeta::DeadDynamicImport)
              && !rec
                .resolved_module
                .is_some_and(|idx| self.link_output.module_table[idx].is_normal())
            {
              return Some("member uses dynamic import");
            }
          }
          ImportKind::AtImport
          | ImportKind::UrlImport
          | ImportKind::NewUrl
          | ImportKind::HotAccept => return Some("member uses an unsupported import kind"),
        }
      }
    }
    if !meta.star_exports_from_external_modules.is_empty() {
      // The finalizer prints `import * as ns from "ext"` and `__reExport` for it inside the
      // module body; an import declaration cannot sit inside a factory.
      return Some("member re-exports an external module's exports");
    }
    // The builtin asset and copy module types stand for an emitted file whose reference is
    // rewritten in `renderChunk`, relative to the chunk that prints it; copies in two carriers
    // would not agree.
    if self.plugin_driver.file_emitter.file_ref_for_module(&module.id).is_some() {
      return Some("member is an emitted asset");
    }
    None
  }

  fn foreign_exported_owner_chunks(
    &self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
  ) -> FxHashSet<ChunkIdx> {
    let mut owners = FxHashSet::default();
    for (chunk_idx, exported) in link_state.index_chunk_exported_symbols.iter_enumerated() {
      if chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx) {
        continue;
      }
      for symbol_ref in exported.keys() {
        let symbol_db = &self.link_output.symbol_db;
        let canonical_ref = symbol_db.canonical_ref_for(*symbol_ref);
        // An export of a CommonJS module's binding is a namespace alias: the file prints
        // `var x = ns.prop` and exports `x`, so the chunk declaring `ns` matters as much as the
        // symbol's own. Ownership is the chunk that prints the declaration, not the chunk of the
        // owning module: an order wrapper's interop symbols belong to the importing module but are
        // declared in the imported CommonJS module's chunk.
        let namespace_ref = symbol_db
          .get(canonical_ref)
          .namespace_alias
          .as_ref()
          .map(|alias| symbol_db.canonical_ref_for(alias.namespace_ref));
        for declared in std::iter::once(canonical_ref).chain(namespace_ref) {
          if let Some(owner_chunk) = link_state.symbol_chunk(declared, symbol_db)
            && owner_chunk != chunk_idx
          {
            owners.insert(owner_chunk);
          }
        }
      }
    }
    owners
  }

  /// The chunks holding an included module that uses direct `eval`.
  fn eval_files(&self, chunk_graph: &ChunkGraph) -> FxHashSet<ChunkIdx> {
    self
      .link_output
      .module_table
      .modules
      .iter()
      .filter_map(|module| module.as_normal())
      .filter(|module| self.link_output.metas[module.idx].is_included && module.meta.has_eval())
      .filter_map(|module| chunk_graph.module_to_chunk[module.idx])
      .collect()
  }

  fn eval_read_chunks(&self, chunk_graph: &ChunkGraph) -> FxHashSet<ChunkIdx> {
    let mut chunks = FxHashSet::default();
    let symbols = &self.link_output.symbol_db;
    for module in
      self.link_output.module_table.modules.iter().filter_map(|module| module.as_normal())
    {
      let meta = &self.link_output.metas[module.idx];
      if !meta.is_included || !module.meta.has_eval() {
        continue;
      }
      for (stmt_info_idx, stmt_info) in self.link_output.stmt_infos[module.idx].iter_enumerated() {
        if !meta.stmt_info_included.has_bit(stmt_info_idx) {
          continue;
        }
        for reference in &stmt_info.referenced_symbols {
          let symbol_ref = match reference {
            SymbolOrMemberExprRef::Symbol(symbol_ref) => Some(*symbol_ref),
            SymbolOrMemberExprRef::MemberExpr(member_expr) => {
              member_expr.represent_symbol_ref(&meta.resolved_member_expr_refs)
            }
          };
          if let Some(symbol_ref) = symbol_ref
            && let Some(owner_chunk) = chunk_graph.module_to_chunk
              [symbols.canonical_ref_resolving_namespace(symbol_ref).owner]
          {
            chunks.insert(owner_chunk);
          }
        }
      }
    }
    chunks
  }
}
