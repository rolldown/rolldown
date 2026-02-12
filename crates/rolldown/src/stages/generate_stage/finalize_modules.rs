use std::sync::Arc;

use rolldown_common::{
  ConcatenateWrappedModuleKind, EcmaViewMeta, PrependRenderedImport,
  RenderedConcatenatedModuleParts, SymbolRef, SymbolRefFlags,
};
use rolldown_utils::{
  index_vec_ext::IndexVecExt as _, indexmap::FxIndexMap, rayon::ParallelIterator as _,
};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::debug_span;

use crate::{
  chunk_graph::ChunkGraph,
  module_finalizers::{FinalizerMutableState, ScopeHoistingFinalizerContext},
};

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn finalize_modules(&mut self, chunk_graph: &mut ChunkGraph) {
    let side_effect_free_function_symbols = self
      .link_output
      .module_table
      .iter()
      .zip(self.link_output.symbol_db.inner().iter())
      .filter_map(|(m, symbol_for_module)| {
        let normal_module = m.as_normal()?;
        let idx = normal_module.idx;
        normal_module
          .meta
          .contains(EcmaViewMeta::TopExportedSideEffectsFreeFunction)
          .then(move || {
            let symbol_for_module = symbol_for_module.as_ref()?;
            Some(symbol_for_module.flags.iter().filter_map(move |(symbol_id, flag)| {
              flag
                .contains(SymbolRefFlags::SideEffectsFreeFunction)
                .then_some(SymbolRef::from((idx, *symbol_id)))
            }))
          })
          .flatten()
      })
      .flatten()
      .collect::<FxHashSet<SymbolRef>>();

    let transfer_parts_rendered_maps = debug_span!("finalize_modules").in_scope(|| {
      self
        .link_output
        .ast_table
        .par_iter_mut_enumerated()
        .filter(|(idx, _ast)| {
          self.link_output.module_table[*idx]
            .as_normal()
            .is_some_and(|m| self.link_output.metas[m.idx].is_included)
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
            modules: &self.link_output.module_table.modules,
            linking_infos: &self.link_output.metas,
            runtime: &self.link_output.runtime,
            options: self.options,
            file_emitter: &self.plugin_driver.file_emitter,
            constant_value_map: &self.link_output.global_constant_symbol_map,
            side_effect_free_function_symbols: &side_effect_free_function_symbols,
            safely_merge_cjs_ns_map: &self.link_output.safely_merge_cjs_ns_map,
            used_symbol_refs: &self.link_output.used_symbol_refs,
          };
          let mutable_state = FinalizerMutableState {
            cur_stmt_index: 0,
            keep_name_statement_to_insert: Vec::new(),
            needs_hosted_top_level_binding: false,
            module_namespace_included: self
              .link_output
              .used_symbol_refs
              .contains(&module.namespace_object_ref),
            transferred_import_record: chunk
              .remove_map
              .get(&module.idx)
              .cloned()
              .map(|idxs| {
                idxs.into_iter().map(|idx| (idx, String::new())).collect::<FxIndexMap<_, _>>()
              })
              .unwrap_or_default(),
            rendered_concatenated_wrapped_module_parts: RenderedConcatenatedModuleParts::default(),
          };

          let concatenated_wrapped_module_kind = ctx.linking_info.concatenated_wrapped_module_kind;
          let (transferred_import_record, rendered_concatenated_wrapped_module_parts) =
            ctx.finalize_normal_module(ast, ast_scope, mutable_state);

          (!transferred_import_record.is_empty()
            || !matches!(concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::None))
          .then_some((idx, transferred_import_record, rendered_concatenated_wrapped_module_parts))
        })
        .collect::<Vec<_>>()
    });

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
      return;
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
  }
}
