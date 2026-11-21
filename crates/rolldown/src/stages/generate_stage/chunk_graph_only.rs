//! Emit chunk-graph metadata without rendering or minifying module code.
//!
//! See `ExperimentalOptions::chunk_graph_only`.

use std::sync::Arc;

use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaAssetMeta, InstantiatedChunk, ModuleId, Output, OutputChunk, RenderedModule,
  RollupRenderedChunk, StrOrBytes,
};
use rolldown_error::BuildResult;
use rustc_hash::FxHashMap;

use crate::{
  BundleOutput, chunk_graph::ChunkGraph, type_alias::IndexChunkToInstance,
  utils::chunk::finalize_chunks::finalize_assets,
};

use super::GenerateStage;

impl GenerateStage<'_> {
  /// Build [`BundleOutput`] whose chunks carry membership and import edges
  /// but empty code. Filename hashing still runs (over the empty content) so
  /// `imports` and `fileName` agree.
  pub(super) async fn emit_chunk_graph_only(
    &self,
    chunk_graph: &ChunkGraph,
  ) -> BuildResult<BundleOutput> {
    let mut index_chunk_to_instance: IndexChunkToInstance =
      oxc_index::index_vec![None; chunk_graph.chunk_table.len()];
    let mut instantiated = IndexVec::new();

    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
        || self.inline_state.is_record(chunk_idx)
      {
        continue;
      }
      let Some(pre) = chunk.pre_rendered_chunk.as_ref() else {
        continue;
      };
      let Some(preliminary_filename) = chunk.preliminary_filename.clone() else {
        continue;
      };

      let mut modules = FxHashMap::default();
      for &module_idx in &chunk.modules {
        let Some(normal) = self.link_output.module_table[module_idx].as_normal() else {
          continue;
        };
        modules.insert(
          ModuleId::clone(&normal.id),
          RenderedModule::new(None, Vec::new(), normal.exec_order),
        );
      }

      let rendered_chunk = Arc::new(RollupRenderedChunk {
        originate_from: chunk_idx,
        name: pre.name.clone(),
        is_entry: pre.is_entry,
        is_dynamic_entry: pre.is_dynamic_entry,
        facade_module_id: pre.facade_module_id.clone(),
        module_ids: pre.module_ids.clone(),
        exports: pre.exports.clone(),
        // Replaced with the content-hashed name by `finalize_assets`.
        filename: ArcStr::clone(&preliminary_filename),
        modules: modules.into(),
        imports: Vec::new(),
        dynamic_imports: Vec::new(),
      });

      let file_dir = self
        .options
        .cwd
        .join(&self.options.out_dir)
        .join(preliminary_filename.as_str())
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| self.options.cwd.join(&self.options.out_dir));

      let ins_idx = instantiated.push(InstantiatedChunk {
        originate_from: chunk_idx,
        content: StrOrBytes::Str(String::new()),
        map: None,
        ecma_meta: Box::new(EcmaAssetMeta {
          rendered_chunk,
          debug_id: 0,
          imports: Vec::new(),
          dynamic_imports: Vec::new(),
          file_dir,
          sourcemap_filename: None,
          preliminary_filename: preliminary_filename.clone(),
        }),
        augment_chunk_hash: None,
        preliminary_filename,
        // No sourcemap file on this path.
        preliminary_sourcemap_filename: None,
        post_banner: None,
        post_footer: None,
      });
      index_chunk_to_instance[chunk_idx] = Some(ins_idx);
    }

    let assets = finalize_assets(
      chunk_graph,
      self.link_output,
      instantiated,
      &index_chunk_to_instance,
      self.options.hash_characters,
      self.options,
      self.resolved_paths.as_ref(),
    )
    .await?;

    let mut output = Vec::with_capacity(assets.len());
    for asset in assets {
      match asset.meta {
        rolldown_common::InstantiationKind::Ecma(ecma_meta) => {
          let rendered_chunk = ecma_meta.rendered_chunk;
          output.push(Output::Chunk(Arc::new(OutputChunk {
            name: rendered_chunk.name.clone(),
            filename: asset.filename,
            code: String::new(),
            is_entry: rendered_chunk.is_entry,
            is_dynamic_entry: rendered_chunk.is_dynamic_entry,
            facade_module_id: rendered_chunk.facade_module_id.clone(),
            modules: rendered_chunk.modules.clone(),
            exports: rendered_chunk.exports.clone(),
            module_ids: rendered_chunk.module_ids.clone(),
            imports: ecma_meta.imports,
            dynamic_imports: ecma_meta.dynamic_imports,
            map: None,
            sourcemap_filename: None,
            preliminary_filename: ecma_meta.preliminary_filename.to_string(),
          })));
        }
        rolldown_common::InstantiationKind::Sourcemap(_) => {}
      }
    }

    output.sort_unstable_by(|a, b| {
      let rank = |o: &Output| match o {
        Output::Asset(_) => 2u8,
        Output::Chunk(c) if c.is_entry => 0u8,
        Output::Chunk(_) => 1u8,
      };
      rank(a).cmp(&rank(b)).then_with(|| a.filename().cmp(b.filename()))
    });

    Ok(BundleOutput { assets: output, warnings: Vec::new(), mangle_cache: None })
  }
}
