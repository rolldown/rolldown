use futures::future::try_join_all;
use rolldown_common::{
  AssetMeta, Output, OutputAsset, OutputChunk, PreliminaryAsset, SourceMapType,
};
use rolldown_error::BuildError;
use sugar_path::SugarPath;

use crate::{
  chunk_graph::ChunkGraph,
  utils::{
    augment_chunk_hash::augment_chunk_hash,
    chunk::{finalize_chunks::finalize_chunks, render_chunk::render_chunk},
    render_chunks::render_chunks,
  },
};

use super::GenerateStage;

impl<'a> GenerateStage<'a> {
  #[allow(clippy::too_many_lines)]
  pub async fn render_chunk_to_assets(
    &mut self,
    chunk_graph: &mut ChunkGraph,
  ) -> anyhow::Result<Vec<Output>> {
    let chunks = try_join_all(
      chunk_graph
        .chunks
        .iter()
        .map(|c| async { render_chunk(c, self.options, self.link_output, chunk_graph).await }),
    )
    .await?;

    let chunks = render_chunks(self.plugin_driver, chunks).await?;

    let chunks = augment_chunk_hash(self.plugin_driver, chunks).await?;

    let chunks = finalize_chunks(chunk_graph, chunks);

    let mut assets = vec![];
    for PreliminaryAsset {
      mut map,
      meta: rendered_chunk,
      content: mut code,
      file_dir,
      preliminary_filename,
      ..
    } in chunks
    {
      if let AssetMeta::Ecma(rendered_chunk) = rendered_chunk {
        if let Some(map) = map.as_mut() {
          map.set_file(&rendered_chunk.filename);

          let map_filename = format!("{}.map", rendered_chunk.filename.as_str());
          let map_path = file_dir.join(&map_filename);

          if let Some(source_map_ignore_list) = &self.options.sourcemap_ignore_list {
            let mut x_google_ignore_list = vec![];
            for (index, source) in map.get_sources().enumerate() {
              if source_map_ignore_list.call(source, map_path.to_string_lossy().as_ref()).await? {
                #[allow(clippy::cast_possible_truncation)]
                x_google_ignore_list.push(index as u32);
              }
            }
            if !x_google_ignore_list.is_empty() {
              map.set_x_google_ignore_list(x_google_ignore_list);
            }
          }

          if let Some(sourcemap_path_transform) = &self.options.sourcemap_path_transform {
            let mut sources = Vec::with_capacity(map.get_sources().count());
            for source in map.get_sources() {
              sources.push(
                sourcemap_path_transform.call(source, map_path.to_string_lossy().as_ref()).await?,
              );
            }
            map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());
          }

          // Normalize the windows path at final.
          let sources =
            map.get_sources().map(|x| x.to_slash_lossy().to_string()).collect::<Vec<_>>();
          map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());

          match self.options.sourcemap {
            SourceMapType::File => {
              let source = match map.to_json_string().map_err(BuildError::sourcemap_error) {
                Ok(source) => source,
                Err(e) => {
                  self.link_output.errors.push(e);
                  continue;
                }
              };
              assets.push(Output::Asset(Box::new(OutputAsset {
                filename: map_filename.clone(),
                source: source.into(),
                name: None,
              })));
              code.push_str(&format!("\n//# sourceMappingURL={map_filename}"));
            }
            SourceMapType::Inline => {
              let data_url = match map.to_data_url().map_err(BuildError::sourcemap_error) {
                Ok(data_url) => data_url,
                Err(e) => {
                  self.link_output.errors.push(e);
                  continue;
                }
              };
              code.push_str(&format!("\n//# sourceMappingURL={data_url}"));
            }
            SourceMapType::Hidden => {}
          }
        }
        let sourcemap_filename =
          map.as_ref().map(|_| format!("{}.map", rendered_chunk.filename.as_str()));
        assets.push(Output::Chunk(Box::new(OutputChunk {
          name: rendered_chunk.name,
          filename: rendered_chunk.filename,
          code,
          is_entry: rendered_chunk.is_entry,
          is_dynamic_entry: rendered_chunk.is_dynamic_entry,
          facade_module_id: rendered_chunk.facade_module_id,
          modules: rendered_chunk.modules,
          exports: rendered_chunk.exports,
          module_ids: rendered_chunk.module_ids,
          imports: rendered_chunk.imports,
          dynamic_imports: rendered_chunk.dynamic_imports,
          map,
          sourcemap_filename,
          preliminary_filename: preliminary_filename.to_string(),
        })));
      }
    }

    Ok(assets)
  }
}
