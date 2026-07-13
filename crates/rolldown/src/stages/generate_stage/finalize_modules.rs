use std::sync::Arc;

use rolldown_common::{ConcatenateWrappedModuleKind, PrependRenderedImport};
use rolldown_error::BuildResult;
use rolldown_utils::{index_vec_ext::IndexVecExt as _, rayon::ParallelIterator as _};
use rustc_hash::FxHashMap;
use tracing::debug_span;

use crate::{
  chunk_graph::ChunkGraph, module_finalizers::ScopeHoistingFinalizerContext,
  type_alias::IndexEcmaAst,
};

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn finalize_modules(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    ast_table: &mut IndexEcmaAst,
    order_state: &super::order_wrap_state::OrderWrapState,
  ) -> BuildResult<()> {
    let has_enum_inlining = self.link_output.has_enum_inlining;
    let has_required_order_runtime = !order_state.required_runtime_helpers().is_empty();
    // Off-strict, lowering never mutates the chunk graph, so the liveness guard cannot fire.
    let strict = self.options.is_strict_execution_order_enabled();

    let finalized = debug_span!("finalize_modules").in_scope(|| {
      ast_table
        .par_iter_mut_enumerated()
        .filter(|(idx, _ast)| {
          self.link_output.module_table[*idx].as_normal().is_some_and(|m| {
            let is_required_order_runtime =
              m.idx == self.link_output.runtime.id() && has_required_order_runtime;
            (self.link_output.metas[m.idx].is_included || is_required_order_runtime)
              && (!strict || chunk_graph.module_is_in_live_chunk(*idx))
          })
        })
        .filter_map(|(idx, ast)| {
          let ast = ast.as_mut()?;
          let module = self.link_output.module_table[idx].as_normal().unwrap();
          let ast_scope = &self.link_output.symbol_db[idx].as_ref().unwrap().ast_scopes;
          let chunk_idx = chunk_graph.module_to_chunk[idx].unwrap();
          let chunk = &chunk_graph.chunk_table[chunk_idx];
          let linking_info = &self.link_output.metas[module.idx];
          let ctx = ScopeHoistingFinalizerContext {
            idx,
            chunk,
            chunk_idx,
            chunk_graph,
            symbol_db: &self.link_output.symbol_db,
            linking_info,
            module,
            stmt_infos: &self.link_output.stmt_infos[idx],
            modules: &self.link_output.module_table.modules,
            linking_infos: &self.link_output.metas,
            order_wrap_state: order_state,
            runtime: &self.link_output.runtime,
            options: self.options,
            file_emitter: &self.plugin_driver.file_emitter,
            constant_value_map: &self.link_output.global_constant_symbol_map,
            safely_merge_cjs_ns_map: &self.link_output.safely_merge_cjs_ns_map,
            retained_export_symbols: &self.link_output.retained_export_symbols,
            resolved_paths: self.resolved_paths.as_ref(),
            has_enum_inlining,
          };

          let concatenated_wrapped_module_kind = ctx.linking_info.concatenated_wrapped_module_kind;
          let (
            transferred_import_record,
            rendered_concatenated_wrapped_module_parts,
            module_errors,
          ) = ctx.finalize_normal_module(ast, ast_scope);

          (!module_errors.is_empty()
            || !transferred_import_record.is_empty()
            || !matches!(concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::None))
          .then_some((
            idx,
            transferred_import_record,
            rendered_concatenated_wrapped_module_parts,
            module_errors,
          ))
        })
        .collect::<Vec<_>>()
    });

    let mut errors = vec![];
    let transfer_parts_rendered_maps = finalized
      .into_iter()
      .filter_map(|(idx, transferred_import_record, rendered_parts, module_errors)| {
        if module_errors.is_empty() {
          return Some((idx, transferred_import_record, rendered_parts));
        }
        errors.extend(module_errors);
        None
      })
      .collect::<Vec<_>>();

    if !errors.is_empty() {
      Err(errors)?;
    }

    let mut normalized_transfer_parts_rendered_maps = FxHashMap::default();
    for (idx, transferred_import_record, rendered_concatenated_module_parts) in
      transfer_parts_rendered_maps
    {
      for (rec_idx, rendered_string) in transferred_import_record {
        normalized_transfer_parts_rendered_maps.insert((idx, rec_idx), rendered_string);
      }
      let chunk_idx = chunk_graph.module_to_chunk[idx].expect("should have chunk idx");
      let chunk = &mut chunk_graph.chunk_table[chunk_idx];
      chunk
        .module_idx_to_render_concatenated_module
        .insert(idx, rendered_concatenated_module_parts);
    }

    if normalized_transfer_parts_rendered_maps.is_empty() {
      return Ok(());
    }
    for chunk in chunk_graph.chunk_table.iter_mut() {
      for (module_idx, recs) in &chunk.insert_map {
        let Some(module) = self.link_output.module_table[*module_idx].as_normal_mut() else {
          continue;
        };
        for (importer_idx, rec_idx) in recs {
          if let Some(rendered_string) =
            normalized_transfer_parts_rendered_maps.get(&(*importer_idx, *rec_idx))
          {
            module
              .ecma_view
              .mutations
              .push(Arc::new(PrependRenderedImport { intro: rendered_string.clone() }));
          }
        }
      }
    }

    Ok(())
  }
}
