use futures::future::try_join_all;
use indexmap::IndexSet;
use oxc::index::{index_vec, IndexVec};
use rolldown_common::{Asset, AssetMeta, Output, OutputAsset, OutputChunk, SourceMapType};
use rolldown_error::BuildDiagnostic;
use sugar_path::SugarPath;

use crate::{
  chunk_graph::ChunkGraph,
  ecmascript::ecma_generator::EcmaGenerator,
  type_alias::{IndexChunkToAssets, IndexPreliminaryAssets},
  types::generator::{GenerateContext, Generator},
  utils::{
    augment_chunk_hash::augment_chunk_hash, chunk::finalize_chunks::finalize_assets,
    render_chunks::render_chunks,
  },
  BundleOutput,
};

use super::GenerateStage;

impl<'a> GenerateStage<'a> {
  #[allow(clippy::too_many_lines)]
  pub async fn render_chunk_to_assets(
    &mut self,
    chunk_graph: &mut ChunkGraph,
  ) -> anyhow::Result<BundleOutput> {
    let mut errors = std::mem::take(&mut self.link_output.errors);
    let mut warnings = std::mem::take(&mut self.link_output.warnings);
    let (mut preliminary_assets, index_chunk_to_assets) =
      self.render_preliminary_assets(chunk_graph, &mut errors, &mut warnings).await?;

    render_chunks(self.plugin_driver, &mut preliminary_assets).await?;

    augment_chunk_hash(self.plugin_driver, &mut preliminary_assets).await?;

    let mut assets = finalize_assets(chunk_graph, preliminary_assets, &index_chunk_to_assets);

    self.minify_assets(&mut assets)?;

    let mut outputs = vec![];
    for Asset {
      mut map,
      meta: rendered_chunk,
      content: mut code,
      file_dir,
      preliminary_filename,
      ..
    } in assets
    {
      if let AssetMeta::Ecma(ecma_meta) = rendered_chunk {
        let rendered_chunk = ecma_meta.rendered_chunk;
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
              let source = map.to_json_string();
              outputs.push(Output::Asset(Box::new(OutputAsset {
                filename: map_filename.clone(),
                source: source.into(),
                original_file_name: None,
                name: None,
              })));
              code.push_str(&format!("\n//# sourceMappingURL={map_filename}"));
            }
            SourceMapType::Inline => {
              let data_url = map.to_data_url();
              code.push_str(&format!("\n//# sourceMappingURL={data_url}"));
            }
            SourceMapType::Hidden => {}
          }
        }
        let sourcemap_filename =
          map.as_ref().map(|_| format!("{}.map", rendered_chunk.filename.as_str()));
        outputs.push(Output::Chunk(Box::new(OutputChunk {
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

    // Make sure order of assets are deterministic
    // TODO: use `preliminary_filename` on `Output::Asset` instead
    outputs.sort_unstable_by(|a, b| a.filename().cmp(b.filename()));

    Ok(BundleOutput { assets: outputs, errors, warnings })
  }

  async fn render_preliminary_assets(
    &self,
    chunk_graph: &ChunkGraph,
    errors: &mut Vec<BuildDiagnostic>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> anyhow::Result<(IndexPreliminaryAssets, IndexChunkToAssets)> {
    let mut index_chunk_to_assets: IndexChunkToAssets =
      index_vec![IndexSet::default(); chunk_graph.chunks.len()];
    let mut index_preliminary_assets: IndexPreliminaryAssets =
      IndexVec::with_capacity(chunk_graph.chunks.len());
    try_join_all(chunk_graph.chunks.iter_enumerated().map(|(chunk_idx, chunk)| async move {
      let mut ctx = GenerateContext {
        chunk_idx,
        chunk,
        options: self.options,
        link_output: self.link_output,
        chunk_graph,
        plugin_driver: self.plugin_driver,
        warnings: vec![],
      };
      EcmaGenerator::render_preliminary_assets(&mut ctx).await
    }))
    .await?
    .into_iter()
    .for_each(|result| match result {
      Ok(generate_output) => {
        generate_output.assets.into_iter().for_each(|asset| {
          let origin_chunk = asset.origin_chunk;
          let asset_idx = index_preliminary_assets.push(asset);
          index_chunk_to_assets[origin_chunk].insert(asset_idx);
        });
        warnings.extend(generate_output.warnings);
      }
      Err(e) => errors.extend(e),
    });

    index_chunk_to_assets.iter_mut().for_each(|assets| {
      assets.sort_by_cached_key(|asset_idx| {
        index_preliminary_assets[*asset_idx].preliminary_filename.as_str()
      });
    });

    Ok((index_preliminary_assets, index_chunk_to_assets))
  }
}
