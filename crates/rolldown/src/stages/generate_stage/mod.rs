use std::sync::Arc;

use futures::future::try_join_all;
use rolldown_common::{
  ChunkKind, FileNameRenderOptions, Output, OutputAsset, OutputChunk, SourceMapType,
};
use rolldown_error::BuildError;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rustc_hash::FxHashSet;

use crate::{
  chunk::ChunkRenderReturn,
  chunk_graph::ChunkGraph,
  error::BatchedResult,
  finalizer::FinalizerContext,
  stages::link_stage::LinkStageOutput,
  utils::{finalize_normal_module, is_in_rust_test_mode, render_chunks::render_chunks},
  SharedOptions,
};

mod code_splitting;
mod compute_cross_chunk_links;

pub struct GenerateStage<'a> {
  link_output: &'a mut LinkStageOutput,
  options: &'a SharedOptions,
  plugin_driver: &'a SharedPluginDriver,
}

impl<'a> GenerateStage<'a> {
  pub fn new(
    link_output: &'a mut LinkStageOutput,
    options: &'a SharedOptions,
    plugin_driver: &'a SharedPluginDriver,
  ) -> Self {
    Self { link_output, options, plugin_driver }
  }

  #[tracing::instrument(skip_all)]
  pub async fn generate(&mut self) -> BatchedResult<Vec<Output>> {
    tracing::info!("Start bundle stage");
    let mut chunk_graph = self.generate_chunks();

    self.generate_chunk_filenames(&mut chunk_graph);
    tracing::info!("generate_chunk_filenames");

    self.compute_cross_chunk_links(&mut chunk_graph);
    tracing::info!("compute_cross_chunk_links");

    chunk_graph.chunks.iter_mut().par_bridge().for_each(|chunk| {
      chunk.de_conflict(self.link_output);
    });

    let ast_table_iter = self.link_output.ast_table.iter_mut_enumerated();
    ast_table_iter
      .par_bridge()
      .filter(|(id, _)| self.link_output.module_table.normal_modules[*id].is_included)
      .for_each(|(id, ast)| {
        let module = &self.link_output.module_table.normal_modules[id];
        let chunk_id = chunk_graph.module_to_chunk[module.id].unwrap();
        let chunk = &chunk_graph.chunks[chunk_id];
        let linking_info = &self.link_output.metas[module.id];
        finalize_normal_module(
          module,
          FinalizerContext {
            canonical_names: &chunk.canonical_names,
            id: module.id,
            symbols: &self.link_output.symbols,
            linking_info,
            module,
            modules: &self.link_output.module_table.normal_modules,
            linking_infos: &self.link_output.metas,
            runtime: &self.link_output.runtime,
            chunk_graph: &chunk_graph,
          },
          ast,
        );
      });
    tracing::info!("finalizing modules");

    let chunks = try_join_all(
      chunk_graph
        .chunks
        .iter()
        .map(|c| async { c.render(self.options, self.link_output, &chunk_graph).await }),
    )
    .await?;

    let mut assets = vec![];

    for ChunkRenderReturn { mut map, rendered_chunk, mut code } in
      render_chunks(self.plugin_driver, chunks).await?
    {
      if let Some(map) = map.as_mut() {
        map.set_file(&rendered_chunk.file_name);
        match self.options.sourcemap {
          SourceMapType::File => {
            let map_file_name = format!("{}.map", rendered_chunk.file_name);
            assets.push(Output::Asset(Arc::new(OutputAsset {
              file_name: map_file_name.clone(),
              source: map.to_json_string().map_err(BuildError::sourcemap_error)?,
            })));
            code.push_str(&format!("\n//# sourceMappingURL={map_file_name}"));
          }
          SourceMapType::Inline => {
            let data_url = map.to_data_url().map_err(BuildError::sourcemap_error)?;
            code.push_str(&format!("\n//# sourceMappingURL={data_url}"));
          }
          SourceMapType::Hidden => {}
        }
      }
      let sourcemap_file_name = map.as_ref().map(|_| format!("{}.map", rendered_chunk.file_name));
      assets.push(Output::Chunk(Arc::new(OutputChunk {
        file_name: rendered_chunk.file_name,
        code,
        is_entry: rendered_chunk.is_entry,
        is_dynamic_entry: rendered_chunk.is_dynamic_entry,
        facade_module_id: rendered_chunk.facade_module_id,
        modules: rendered_chunk.modules,
        exports: rendered_chunk.exports,
        module_ids: rendered_chunk.module_ids,
        map,
        sourcemap_file_name,
      })));
    }

    tracing::info!("rendered chunks");

    Ok(assets)
  }

  fn generate_chunk_filenames(&self, chunk_graph: &mut ChunkGraph) {
    let mut used_chunk_names = FxHashSet::default();
    chunk_graph.chunks.iter_mut().for_each(|chunk| {
      let runtime_id = self.link_output.runtime.id();

      let file_name_tmp = chunk.file_name_template(self.options);
      let chunk_name =
        if is_in_rust_test_mode() && chunk.modules.first().copied() == Some(runtime_id) {
          "$runtime$".to_string()
        } else {
          chunk.name.clone().unwrap_or_else(|| {
            let module_id =
              if let ChunkKind::EntryPoint { module: entry_module_id, is_user_defined, .. } =
                &chunk.kind
              {
                debug_assert!(
                  !*is_user_defined,
                  "User-defined entry point should always have a name"
                );
                *entry_module_id
              } else {
                // TODO: we currently use the first executed module to calculate the chunk name for common chunks
                // This is not perfect, should investigate more to find a better solution
                chunk.modules.first().copied().unwrap()
              };
            let module = &self.link_output.module_table.normal_modules[module_id];
            module.resource_id.expect_file().unique(&self.options.cwd)
          })
        };

      let mut chunk_name = chunk_name;
      while used_chunk_names.contains(&chunk_name) {
        chunk_name = format!("{}-{}", chunk_name, used_chunk_names.len());
      }
      used_chunk_names.insert(chunk_name.clone());

      chunk.file_name =
        Some(file_name_tmp.render(&FileNameRenderOptions { name: Some(&chunk_name) }));
    });
  }
}
