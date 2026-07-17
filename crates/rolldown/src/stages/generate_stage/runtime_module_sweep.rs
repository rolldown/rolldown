use rolldown_common::{
  ImportRecordMeta, Module, ModuleIdx, NormalModule, PostChunkOptimizationOperation, RuntimeHelper,
  StmtInfos, SymbolOrMemberExprRef, UsedSymbolRefsBuilder,
};
use rolldown_utils::IndexBitSet;

use crate::{chunk_graph::ChunkGraph, types::linking_metadata::LinkingMetadata};

use super::GenerateStage;

impl GenerateStage<'_> {
  /// Drop the runtime module when nothing in the final output will reference it.
  ///
  /// Tree-shaking includes runtime helpers while linking, before chunks exist. Some of the
  /// link-time reasons are later invalidated by `find_entry_level_external_module`: a star
  /// re-export chain that ends at an external module is flattened to a chunk-level
  /// `export * from '<external>'` (or the CJS equivalent), `has_dynamic_exports` is
  /// re-propagated to `false`, and `finalized_module_namespace_ref_usage` drops the namespace
  /// objects that only existed to serve that chain. The module finalizer then emits no
  /// `__reExport`/`__exportAll` call — but the runtime module is already included and already
  /// assigned to a chunk, so it used to ship as a dead chunk plus bare imports of it (#9374).
  ///
  /// This sweep re-derives the runtime demand from the same post-walk-back facts the finalizer
  /// reads. It is all-or-nothing: if any included statement still demands any helper, everything
  /// stays exactly as tree-shaking decided; only a runtime module with zero remaining demand is
  /// un-included and stripped from its chunk.
  ///
  /// Must run after `find_entry_level_external_module` and
  /// `finalized_module_namespace_ref_usage` (the facts it reads), and before the chunk
  /// exec-order assignment and `compute_cross_chunk_links` (the consumers of what it mutates).
  ///
  /// See internal-docs/code-splitting/implementation.md ("Unused-Runtime Sweep") for the
  /// pipeline position and the liveness invariants this pass relies on.
  pub fn sweep_unused_runtime_module(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    used_symbol_refs: &mut UsedSymbolRefsBuilder,
  ) {
    // Without tree-shaking, inclusion is not demand-based; leave everything in place.
    if self.options.treeshake.is_none() {
      return;
    }
    let runtime_idx = self.link_output.runtime.id();
    if !self.link_output.metas[runtime_idx].is_included {
      return;
    }
    let Some(runtime_module) = self.link_output.module_table[runtime_idx].as_normal() else {
      return;
    };
    // A side-effectful runtime (dev/HMR mode, or a plugin transformed it) must load no matter
    // whether its helpers are referenced; see `has_side_effectful_runtime_dep`.
    if runtime_module.side_effects.has_side_effects() {
      return;
    }
    if self.runtime_helpers_still_demanded(runtime_idx) {
      return;
    }

    let runtime_stmt_count = self.link_output.stmt_infos[runtime_idx].len();
    let runtime_meta = &mut self.link_output.metas[runtime_idx];
    runtime_meta.is_included = false;
    runtime_meta.stmt_info_included = IndexBitSet::new(runtime_stmt_count);
    runtime_meta.depended_runtime_helper = RuntimeHelper::default();
    // Stale references to runtime symbols may survive on included statements (e.g. a namespace
    // statement that `finalized_module_namespace_ref_usage` decided not to render keeps its
    // `__exportAll` reference, and chunk-level `depended_runtime_helper` flags keep their bits).
    // Downstream passes filter those against `used_symbol_refs`, so purging the runtime's
    // symbols here is what actually severs every cross-chunk edge to it.
    used_symbol_refs.remove_owned_by(runtime_idx);

    if let Some(chunk_idx) = chunk_graph.module_to_chunk[runtime_idx] {
      chunk_graph.module_to_chunk[runtime_idx] = None;
      let chunk = &mut chunk_graph.chunk_table[chunk_idx];
      chunk.modules.retain(|module_idx| *module_idx != runtime_idx);
      if chunk.modules.is_empty()
        && !chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
      {
        chunk_graph
          .post_chunk_optimization_operations
          .insert(chunk_idx, PostChunkOptimizationOperation::Removed);
      }
    }
  }

  /// Re-derive whether any included module still needs a runtime helper, using the
  /// post-walk-back facts the module finalizer will render from. Conservative by design:
  /// any uncertain case answers `true` (keep the runtime), which is at worst today's
  /// behavior.
  fn runtime_helpers_still_demanded(&self, runtime_idx: ModuleIdx) -> bool {
    let link = &self.link_output;
    let symbol_db = &link.symbol_db;
    for (module_idx, module) in link.module_table.modules.iter_enumerated() {
      if module_idx == runtime_idx {
        continue;
      }
      let Some(module) = module.as_normal() else {
        continue;
      };
      let meta = &link.metas[module_idx];
      if !meta.is_included {
        continue;
      }

      // Helper-flag channel (`ReferenceNeededSymbolsPass` requirements folded by tree-shaking).
      // `ReExport` is the one flag whose link-time justification the walk-back can invalidate:
      // it was registered for `export * from './normal'` statements because the importee had
      // dynamic exports at link time. The finalizer emits the `__reExport` call only when the
      // importee still has them (see `remove_unused_top_level_stmt`), so mirror that here.
      // Every other flag's condition (wrap kinds, formats, interop) is stable after linking.
      let mut helpers = meta.depended_runtime_helper;
      if helpers.contains(RuntimeHelper::ReExport)
        && !Self::star_re_export_still_demanded(link, module, meta)
      {
        helpers.remove(RuntimeHelper::ReExport);
      }
      if !helpers.is_empty() {
        return true;
      }

      // Namespace-object channel: `CreateSyntheticExportStatementsPass` put `__exportAll` (and
      // `__reExport` for star re-exports of externals) on the namespace statement. The
      // statement renders only when `finalized_module_namespace_ref_usage` retained the
      // namespace; mirror `generate_declaration_of_module_namespace_object`.
      if meta.namespace_included {
        if !meta.is_canonical_exports_empty() || self.options.generated_code.symbols {
          // `__exportAll` (over-approximate: the finalizer may still render `var ns = {}` if
          // every export got trimmed from the object literal, but keeping the runtime for
          // that corner is the safe direction).
          return true;
        }
        if meta.star_exports_from_external_modules.iter().any(|rec_idx| {
          meta.ns_star_external_re_export_emitted(
            module.import_records[*rec_idx].meta,
            self.options.format,
          )
        }) {
          // `__reExport(ns, <external>)`
          return true;
        }
      }

      // Direct-reference channel: statements whose `referenced_symbols` point into the runtime
      // module (wrapper statements referencing `__esm`/`__commonJS`, `__require` rewrites, …).
      // The namespace statement is skipped — it is exactly the statement whose rendering the
      // generate stage re-decides, and it is handled above.
      for (stmt_idx, stmt_info) in link.stmt_infos[module_idx].iter_enumerated() {
        if stmt_idx == StmtInfos::NAMESPACE_STMT_IDX || !meta.stmt_info_included.has_bit(stmt_idx) {
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
            && symbol_db.canonical_ref_for(symbol_ref).owner == runtime_idx
          {
            return true;
          }
        }
      }

      // Entry-chunk channel: symbols referenced by generated entry-chunk code without a
      // carrying statement.
      if meta
        .referenced_symbols_by_entry_point_chunk
        .iter()
        .any(|(symbol_ref, _)| symbol_db.canonical_ref_for(*symbol_ref).owner == runtime_idx)
      {
        return true;
      }
    }
    false
  }

  /// Whether the finalizer will still emit a `__reExport` call for one of `module`'s included
  /// `export * from './normal'` statements: true iff some star importee still has dynamic
  /// exports after `find_entry_level_external_module` re-propagated the flag. A CommonJS
  /// importee always keeps `has_dynamic_exports`, so this also covers the wrapped-CJS star
  /// re-export registration.
  fn star_re_export_still_demanded(
    link: &crate::stages::link_stage::LinkStageOutput,
    module: &NormalModule,
    meta: &LinkingMetadata,
  ) -> bool {
    link.stmt_infos[module.idx].iter_enumerated().any(|(stmt_idx, stmt_info)| {
      if !meta.stmt_info_included.has_bit(stmt_idx) {
        return false;
      }
      stmt_info.import_records.iter().any(|rec_idx| {
        let rec = &module.import_records[*rec_idx];
        if !rec.meta.contains(ImportRecordMeta::IsExportStar) {
          return false;
        }
        let Some(importee_idx) = rec.resolved_module else {
          return false;
        };
        matches!(link.module_table[importee_idx], Module::Normal(_))
          && link.metas[importee_idx].has_dynamic_exports
      })
    })
  }
}
