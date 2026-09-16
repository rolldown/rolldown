use itertools::Itertools;
use oxc::ast::ast::ImportMeta;
use oxc::ast_visit::{VisitJs, walk_js};
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkReasonType, ConcatenateWrappedModuleKind, ImportKind,
  ImportRecordMeta, ModuleIdx, RuntimeHelper, StmtInfoIdx, StmtInfoMeta, SymbolOrMemberExprRef,
  UsedSymbolRefsBuilder, WrapKind,
};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::FxHashSet;

use super::{LOG_TARGET, place::compute_placement};
use crate::{
  chunk_graph::ChunkGraph,
  stages::generate_stage::{
    GenerateStage,
    compute_cross_chunk_links::{CrossChunkLinkState, FinalEsmInitMetadataAvailability},
    order_wrap_state::OrderWrapState,
  },
};

struct ImportMetaFinder {
  found: bool,
}

impl VisitJs<'_> for ImportMetaFinder {
  fn visit_import_meta(&mut self, _it: &ImportMeta) {
    self.found = true;
  }
}

/// Chunk-level facts every candidate is checked against.
struct SelectionFacts<'a> {
  link_state: &'a CrossChunkLinkState,
  importer_count: &'a IndexVec<ChunkIdx, usize>,
  runtime_chunk: Option<ChunkIdx>,
  /// Chunks some `import()` resolves to, plus chunks hosting a collapsed dynamic-entry facade.
  dynamic_targets: &'a FxHashSet<ChunkIdx>,
  /// Chunks owning a symbol that another chunk re-exports with `export { x }`: an ESM export
  /// needs a local binding, which a bridge property read cannot provide.
  foreign_exported_owner_chunks: &'a FxHashSet<ChunkIdx>,
  /// Chunks whose symbols a module with direct `eval` references by name.
  eval_read_chunks: &'a FxHashSet<ChunkIdx>,
  max_size: u64,
}

impl GenerateStage<'_> {
  /// The selection point: order lowering is final, so "every member has a wrapper" is a fact,
  /// and the unused-runtime sweep has not run, so the runtime demand registered here is what the
  /// sweep sees. Reads a provisional, read-only cross-chunk link pass for the edges.
  pub(in crate::stages::generate_stage) fn select_inline_common_chunks(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    order_state: &mut OrderWrapState,
    used_symbol_refs_builder: &UsedSymbolRefsBuilder,
  ) {
    let Some(max_size) = self
      .options
      .inline_common_chunks
      .as_ref()
      .filter(|options| options.is_enabled())
      .map(|options| options.max_size)
    else {
      return;
    };

    let final_esm_init_metadata =
      self.compute_wrapped_esm_init_metadata(&self.ast_table, chunk_graph, order_state);
    let link_state = self.compute_cross_chunk_link_state(
      chunk_graph,
      used_symbol_refs_builder.view(),
      order_state,
      FinalEsmInitMetadataAvailability::Sealed(&final_esm_init_metadata),
    );

    let is_live = |chunk_idx: ChunkIdx| {
      !chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
    };
    let static_importees: IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> = chunk_graph
      .chunk_table
      .iter_enumerated()
      .map(|(chunk_idx, chunk)| {
        if !is_live(chunk_idx) {
          return FxHashSet::default();
        }
        link_state.index_imports_from_other_chunks[chunk_idx]
          .keys()
          .copied()
          .chain(chunk.imports_from_other_chunks.keys().copied())
          .filter(|importee| *importee != chunk_idx)
          .collect()
      })
      .collect();
    let mut importer_count: IndexVec<ChunkIdx, usize> =
      index_vec![0; chunk_graph.chunk_table.len()];
    for importees in &static_importees {
      for importee in importees {
        importer_count[*importee] += 1;
      }
    }

    let runtime_idx = self.link_output.runtime.id();
    let runtime_chunk = chunk_graph.module_to_chunk[runtime_idx];
    let dynamic_targets = link_state
      .index_cross_chunk_dynamic_imports
      .iter()
      .flatten()
      .copied()
      .chain(chunk_graph.common_chunk_exported_facade_chunk_namespace.keys().copied())
      .chain(chunk_graph.entry_module_to_entry_chunk.values().copied())
      .collect::<FxHashSet<_>>();
    let foreign_exported_owner_chunks =
      self.foreign_exported_owner_chunks(chunk_graph, &link_state);
    let eval_read_chunks = self.eval_read_chunks(chunk_graph);
    let facts = SelectionFacts {
      link_state: &link_state,
      importer_count: &importer_count,
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
      if !is_live(chunk_idx) || Some(chunk_idx) == runtime_chunk {
        continue;
      }
      graph.add_node(chunk_idx);
      for importee in importees {
        if Some(*importee) != runtime_chunk && is_live(*importee) {
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
    let placement = loop {
      let placement = compute_placement(chunk_graph, &static_importees, &records);
      let orphans =
        records.iter().copied().filter(|record| !placement.is_carried(*record)).collect_vec();
      if orphans.is_empty() {
        break placement;
      }
      for orphan in orphans {
        records.shift_remove(&orphan);
        tracing::debug!(
          target: LOG_TARGET,
          chunk = orphan.raw(),
          modules = ?self.module_ids_of(&chunk_graph.chunk_table[orphan]),
          reason = "no file reads it",
          "kept as file"
        );
      }
    };
    if records.is_empty() {
      tracing::debug!(target: LOG_TARGET, "no common chunk selected");
      return;
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

    // The registry lives in the runtime module, so every carrier imports the runtime chunk. Under
    // the option's preconditions every member is wrapped and the runtime already sits in a
    // standalone chunk (`try_merge_runtime_chunk` never merges it while the feature is on); the
    // branch below mirrors the order-wrap path for a runtime that has no standalone chunk yet, so
    // the registry demand always has a chunk to import from.
    let runtime_is_standalone = chunk_graph.module_to_chunk[runtime_idx]
      .is_some_and(|chunk_idx| chunk_graph.chunk_table[chunk_idx].modules.len() == 1);
    if !runtime_is_standalone {
      self.ensure_runtime_module_for_order_wraps(chunk_graph);
      chunk_graph.sort_chunk_modules(self.link_output, self.options);
      self.renumber_live_chunks(chunk_graph);
    }
    let runtime_chunk =
      chunk_graph.module_to_chunk[runtime_idx].expect("runtime module should sit in a chunk");

    let mut carriers = placement.carried.keys().copied().collect_vec();
    carriers.sort_unstable();
    for carrier in carriers {
      order_state
        .insert_runtime_helper_demand(carrier, RuntimeHelper::Share | RuntimeHelper::ShareRequire);
    }
    for record in &records {
      let mut helpers = RuntimeHelper::ShareExport;
      if placement.readers.contains_key(record) {
        helpers |= RuntimeHelper::ShareRequire;
      }
      order_state.insert_runtime_helper_demand(*record, helpers);
    }
    order_state.compute_runtime_symbol_closure(
      &self.link_output.runtime,
      &self.link_output.stmt_infos[runtime_idx],
      &self.link_output.symbol_db,
    );

    self.inline_state.set_selection(records, placement, runtime_chunk);
  }

  fn module_ids_of(&self, chunk: &Chunk) -> Vec<String> {
    chunk
      .modules
      .iter()
      .map(|module_idx| self.link_output.module_table[*module_idx].stable_id().to_string())
      .collect()
  }

  fn chunk_size(&self, chunk: &Chunk) -> u64 {
    chunk
      .modules
      .iter()
      .map(|module_idx| {
        u64::try_from(self.link_output.module_table[*module_idx].size()).unwrap_or(u64::MAX)
      })
      .fold(0u64, u64::saturating_add)
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
    if Some(chunk_idx) == facts.runtime_chunk {
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
    if facts.importer_count[chunk_idx] == 0 {
      return Some("no static importer");
    }
    if facts.foreign_exported_owner_chunks.contains(&chunk_idx) {
      return Some("another chunk re-exports one of its symbols");
    }
    if facts.eval_read_chunks.contains(&chunk_idx) {
      return Some("a module with direct eval references its symbols");
    }
    if !chunk.entry_level_external_module_idx.is_empty()
      || !facts.link_state.index_chunk_direct_imports_from_external_modules[chunk_idx].is_empty()
      || !facts.link_state.index_chunk_indirect_imports_from_external_modules[chunk_idx].is_empty()
      || !facts.link_state.index_chunk_dynamic_imports_from_external_modules[chunk_idx].is_empty()
    {
      return Some("imports an external module");
    }
    for module_idx in chunk.modules.iter().copied() {
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
      if order_state.esm_init_target(module_idx, meta).is_none()
        && !matches!(meta.wrap_kind(), WrapKind::Cjs)
      {
        return Some("member has no wrapper");
      }
      if module
        .ecma_view
        .rolldown_file_url_references
        .iter()
        .any(|reference| meta.stmt_info_included.has_bit(reference.stmt_info_idx))
      {
        return Some("member uses import.meta.ROLLDOWN_FILE_URL_*");
      }
      for (stmt_info_idx, stmt_info) in self.link_output.stmt_infos[module_idx].iter_enumerated() {
        if !meta.stmt_info_included.has_bit(stmt_info_idx) {
          continue;
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
              if !rec.meta.contains(ImportRecordMeta::DeadDynamicImport) {
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
      if self.retained_code_uses_import_meta(module_idx) {
        return Some("member uses import.meta");
      }
    }
    if self.chunk_size(chunk) >= facts.max_size {
      return Some("not smaller than maxSize");
    }
    None
  }

  fn retained_code_uses_import_meta(&self, module_idx: ModuleIdx) -> bool {
    let Some(ast) = self.ast_table[module_idx].as_ref() else {
      return false;
    };
    let meta = &self.link_output.metas[module_idx];
    let mut finder = ImportMetaFinder { found: false };
    // `program.body[i]` is described by `stmt_infos[i + 1]`; index 0 is the namespace statement.
    for (index, stmt) in ast.program().body.iter().enumerate() {
      if !meta.stmt_info_included.has_bit(StmtInfoIdx::from_usize(index + 1)) {
        continue;
      }
      walk_js::walk_statement(&mut finder, stmt);
      if finder.found {
        return true;
      }
    }
    false
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
        let canonical_ref = self.link_output.symbol_db.canonical_ref_for(*symbol_ref);
        if let Some(owner_chunk) = chunk_graph.module_to_chunk[canonical_ref.owner]
          && owner_chunk != chunk_idx
        {
          owners.insert(owner_chunk);
        }
      }
    }
    owners
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
