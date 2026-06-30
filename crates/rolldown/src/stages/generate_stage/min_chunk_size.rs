//! `experimental.minChunkSize`: webpack/rspack `splitChunks.minSize`-style merging.
//!
//! A small, side-effect-free common "leaf" chunk (its modules have no imports of
//! their own) is *duplicated* into each chunk that imports it instead of being
//! emitted as a standalone shared chunk. This reduces chunk / request count for
//! the price of duplicating a tiny module.
//!
//! Correctness relies on three facts (see
//! `internal-docs/min-size-chunk-merging/design.md`):
//! - leaves have no outgoing cross-chunk refs, so a single finalized AST has
//!   nothing chunk-specific except its own declared-symbol names;
//! - those names are pinned to one globally-unique name used by every chunk
//!   (`ChunkGraph::duplicated_leaf_pinned_names`), so the one finalized AST is
//!   valid everywhere;
//! - a duplicated leaf is recorded in `ChunkGraph::duplicated_leaf_modules` and
//!   treated as **local to every chunk that references it** by
//!   `compute_cross_chunk_links` and the module finalizer.
//!
//! This pass runs after chunk formation and before `compute_cross_chunk_links`.

use oxc_str::CompactStr;
use rolldown_common::{
  ChunkIdx, ChunkKind, EcmaViewMeta, ImportKind, ImportRecordMeta, ModuleIdx,
  PostChunkOptimizationOperation, SymbolRef, TaggedSymbolRef,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{chunk_graph::ChunkGraph, utils::renamer::Renamer};

use super::GenerateStage;

impl GenerateStage<'_> {
  pub(super) fn merge_small_common_leaf_chunks(&mut self, chunk_graph: &mut ChunkGraph) {
    let Some(min_size) = self.options.experimental.min_chunk_size() else {
      return;
    };

    // The duplicated-leaf "local everywhere" rule is only wired into the ESM
    // finalizer and `compute_cross_chunk_links` paths. The CJS/IIFE/UMD
    // reference-resolution paths (the `module_finalizers` CJS branch and
    // `types/generator.rs`) still classify a duplicated leaf living in a
    // non-primary chunk as cross-chunk, which resolves incorrectly or panics
    // indexing a require binding that was deliberately never populated for the leaf.
    // Restrict the optimization to ESM output until those paths honor
    // `duplicated_leaf_modules`. (Wrapped / `require()`d leaves carry a runtime
    // import record, so the `import_records.is_empty()` leaf check below already
    // excludes them.)
    if !self.options.format.is_esm() {
      return;
    }

    // 1. Modules re-exported anywhere are conservatively excluded: their symbols
    //    may be referenced through cross-chunk re-export chains that the direct
    //    import-edge scan below would miss.
    let reexported_modules = self.collect_reexported_modules();

    // 2. Eligible chunks: Common (not runtime), all included side-effect-free
    //    leaves, not re-exported, total source size under the threshold.
    let runtime_chunk = chunk_graph.module_to_chunk[self.link_output.runtime.id()];
    let eligible: Vec<ChunkIdx> = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(chunk_idx, chunk)| {
        matches!(chunk.kind, ChunkKind::Common)
          && Some(*chunk_idx) != runtime_chunk
          && !chunk.modules.is_empty()
          && chunk_graph.post_chunk_optimization_operations.get(chunk_idx).copied()
            != Some(PostChunkOptimizationOperation::Removed)
          && self.is_small_leaf_chunk(&chunk.modules, &reexported_modules, min_size)
      })
      .map(|(chunk_idx, _)| chunk_idx)
      .collect();
    if eligible.is_empty() {
      return;
    }

    // 3. Map each eligible leaf module to its (soon-to-be-removed) owner chunk,
    //    then find the importing chunks via direct import edges.
    let mut module_eligible_chunk: FxHashMap<ModuleIdx, ChunkIdx> = FxHashMap::default();
    for &s in &eligible {
      for &m in &chunk_graph.chunk_table[s].modules {
        module_eligible_chunk.insert(m, s);
      }
    }
    let importers = self.collect_importers(chunk_graph, &module_eligible_chunk);

    // 4. Duplicate each eligible leaf chunk into its importers.
    let mut duplicated_leaf_modules: FxHashSet<ModuleIdx> = FxHashSet::default();
    let mut leaf_symbols: FxHashSet<SymbolRef> = FxHashSet::default();
    let mut leaf_primary_chunk: FxHashMap<ModuleIdx, ChunkIdx> = FxHashMap::default();

    for &s in &eligible {
      let Some(importer_set) = importers.get(&s) else {
        continue;
      };
      if importer_set.is_empty() {
        continue;
      }
      let mut importer_chunks: Vec<ChunkIdx> = importer_set.iter().copied().collect();
      importer_chunks.sort_unstable();
      let primary = importer_chunks[0];

      let s_modules = chunk_graph.chunk_table[s].modules.clone();

      // A duplicated-leaf symbol is pinned to its own name and force-reserved in
      // every importer chunk *before* that chunk's unresolved (free) globals are
      // reserved, and a free reference is never renamed. So if a leaf-declared name
      // also appears as an unresolved global reference in an importer chunk, the
      // copied declaration would silently shadow that global (wrong runtime value,
      // no error). Leaving the leaf as a standalone shared chunk is always correct,
      // so skip duplicating it in that case.
      if self.leaf_name_shadows_importer_global(&s_modules, &importer_chunks, chunk_graph) {
        continue;
      }

      let s_runtime_helper = chunk_graph.chunk_table[s].depended_runtime_helper;
      for &c in &importer_chunks {
        for &m in &s_modules {
          chunk_graph.chunk_table[c].modules.push(m);
        }
        chunk_graph.chunk_table[c].depended_runtime_helper.insert(s_runtime_helper);
      }

      for &m in &s_modules {
        duplicated_leaf_modules.insert(m);
        leaf_primary_chunk.insert(m, primary);
        self.collect_declared_symbols(m, &mut leaf_symbols);
      }

      // Tombstone the now-empty shared chunk.
      chunk_graph.chunk_table[s].modules.clear();
      chunk_graph
        .post_chunk_optimization_operations
        .insert(s, PostChunkOptimizationOperation::Removed);
    }

    if duplicated_leaf_modules.is_empty() {
      return;
    }

    // 5. Compute globally-unique pinned names for every duplicated-leaf symbol.
    let mut ordered: Vec<SymbolRef> = leaf_symbols.into_iter().collect();
    ordered.sort_unstable_by(|a, b| {
      let a_key = self.link_output.module_table[a.owner].exec_order();
      let b_key = self.link_output.module_table[b.owner].exec_order();
      a_key
        .cmp(&b_key)
        .then_with(|| a.name(&self.link_output.symbol_db).cmp(b.name(&self.link_output.symbol_db)))
    });
    let mut renamer = Renamer::new(None, &self.link_output.symbol_db, self.options.format);
    for &sym in &ordered {
      renamer.add_symbol_in_root_scope(sym, true);
    }
    let pinned_names = renamer.into_canonical_names();

    // 6. Point duplicated leaves' module/symbol chunk metadata at the primary
    //    importer so the one finalized AST renders with the pinned names, and the
    //    `compute_cross_chunk_links` symbol-assignment skip stays consistent.
    for (&m, &primary) in &leaf_primary_chunk {
      chunk_graph.module_to_chunk[m] = Some(primary);
    }
    {
      let symbol_db = &mut self.link_output.symbol_db;
      for &sym in &ordered {
        if let Some(&primary) = leaf_primary_chunk.get(&sym.owner) {
          let canonical = sym.canonical_ref(symbol_db);
          symbol_db.get_mut(canonical).chunk_idx = Some(primary);
        }
      }
    }

    chunk_graph.duplicated_leaf_modules = duplicated_leaf_modules;
    chunk_graph.duplicated_leaf_pinned_names = pinned_names;

    // 7. Re-sort modules within every chunk so the deconflicter's ascending
    //    exec-order invariant holds after the insertions.
    chunk_graph.sort_chunk_modules(self.link_output, self.options);
  }

  fn collect_reexported_modules(&self) -> FxHashSet<ModuleIdx> {
    let mut reexported = FxHashSet::default();
    // Scan ALL modules (not just included ones): a pure re-export pass-through
    // module (`export { k } from './util'`) is often tree-shaken out
    // (`is_included == false`), yet a consumer can still reference the leaf's
    // symbol through it. Excluding such leaves keeps the direct-import-edge
    // importer scan sound.
    for (_m_idx, module) in self.link_output.module_table.modules.iter_enumerated() {
      let Some(normal) = module.as_normal() else {
        continue;
      };
      for rec in &normal.import_records {
        if rec.meta.contains(ImportRecordMeta::IsExportStar) {
          if let Some(t) = rec.resolved_module {
            reexported.insert(t);
          }
        }
      }
      for local_export in normal.named_exports.values() {
        if let Some(named_import) = normal.named_imports.get(&local_export.referenced) {
          if let Some(t) = normal.import_records[named_import.record_idx].resolved_module {
            reexported.insert(t);
          }
        }
      }
    }
    reexported
  }

  /// True if any symbol declared by the candidate leaf has a name that also
  /// appears as an unresolved (free) global reference in one of its importer
  /// chunks. Duplicating such a leaf would shadow the importer's global, so the
  /// caller keeps it as a standalone shared chunk instead (always correct; at
  /// worst a missed optimization). Conservative: it tests the leaf symbols' own
  /// names, which is what they are pinned to (a within-leaf-set suffix only moves
  /// the pinned name further away from the global, never toward it).
  fn leaf_name_shadows_importer_global(
    &self,
    leaf_modules: &[ModuleIdx],
    importer_chunks: &[ChunkIdx],
    chunk_graph: &ChunkGraph,
  ) -> bool {
    let mut leaf_symbols: FxHashSet<SymbolRef> = FxHashSet::default();
    for &m in leaf_modules {
      self.collect_declared_symbols(m, &mut leaf_symbols);
    }
    let leaf_names: FxHashSet<CompactStr> = leaf_symbols
      .iter()
      .map(|sym| CompactStr::new(sym.name(&self.link_output.symbol_db)))
      .collect();
    if leaf_names.is_empty() {
      return false;
    }
    importer_chunks.iter().any(|&c| {
      chunk_graph.chunk_table[c].modules.iter().any(|&idx| {
        self.link_output.symbol_db[idx].as_ref().is_some_and(|db| {
          db.ast_scopes
            .scoping()
            .root_unresolved_references()
            .keys()
            .any(|name| leaf_names.contains(&CompactStr::new(name)))
        })
      })
    })
  }

  #[expect(clippy::cast_precision_loss)] // module byte sizes are well within f64 range
  fn is_small_leaf_chunk(
    &self,
    modules: &[ModuleIdx],
    reexported_modules: &FxHashSet<ModuleIdx>,
    min_size: f64,
  ) -> bool {
    let mut size = 0.0f64;
    let all_leaves = modules.iter().all(|&m| {
      let Some(normal) = self.link_output.module_table[m].as_normal() else {
        return false;
      };
      size += self.link_output.module_table[m].size() as f64;
      self.link_output.metas[m].is_included
        && normal.import_records.is_empty()
        && !self.link_output.module_table[m].side_effects().has_side_effects()
        && !normal.meta.contains(EcmaViewMeta::ExecutionOrderSensitive)
        && !reexported_modules.contains(&m)
    });
    all_leaves && size < min_size
  }

  fn collect_importers(
    &self,
    chunk_graph: &ChunkGraph,
    module_eligible_chunk: &FxHashMap<ModuleIdx, ChunkIdx>,
  ) -> FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> {
    let mut importers: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> = FxHashMap::default();
    for (m_idx, module) in self.link_output.module_table.modules.iter_enumerated() {
      let Some(normal) = module.as_normal() else {
        continue;
      };
      if !self.link_output.metas[m_idx].is_included {
        continue;
      }
      let Some(importer_chunk) = chunk_graph.module_to_chunk[m_idx] else {
        continue;
      };
      for rec in &normal.import_records {
        if rec.kind != ImportKind::Import {
          continue;
        }
        let Some(importee) = rec.resolved_module else {
          continue;
        };
        if let Some(&s) = module_eligible_chunk.get(&importee) {
          if s != importer_chunk {
            importers.entry(s).or_default().insert(importer_chunk);
          }
        }
      }
    }
    importers
  }

  fn collect_declared_symbols(&self, module_idx: ModuleIdx, out: &mut FxHashSet<SymbolRef>) {
    let Some(normal) = self.link_output.module_table[module_idx].as_normal() else {
      return;
    };
    let meta = &self.link_output.metas[module_idx];
    for (stmt_idx, stmt_info) in normal.stmt_infos.iter_enumerated() {
      if !meta.stmt_info_included.has_bit(stmt_idx) {
        continue;
      }
      for declared in &stmt_info.declared_symbols {
        if matches!(declared, TaggedSymbolRef::Normal(_)) {
          let sym = declared.inner();
          if sym.owner == module_idx {
            out.insert(sym);
          }
        }
      }
    }
  }
}
