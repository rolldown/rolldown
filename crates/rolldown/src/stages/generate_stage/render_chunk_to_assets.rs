use std::path::Path;

use futures::future::try_join_all;
use indexmap::IndexSet;
use oxc::index::{index_vec, IndexVec};
use rolldown_common::{Asset, InstantiationKind, Output, OutputAsset, OutputChunk, SourceMapType};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_error::BuildDiagnostic;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use sugar_path::SugarPath;

use crate::{
  chunk_graph::ChunkGraph,
  css::css_generator::CssGenerator,
  ecmascript::ecma_generator::EcmaGenerator,
  type_alias::{IndexChunkToAssets, IndexInstantiatedChunks},
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
    let (mut instantiated_chunks, index_chunk_to_assets) =
      self.instantiate_chunks(chunk_graph, &mut errors, &mut warnings).await?;

    render_chunks(self.plugin_driver, &mut instantiated_chunks).await?;

    augment_chunk_hash(self.plugin_driver, &mut instantiated_chunks).await?;

    let mut assets = finalize_assets(chunk_graph, instantiated_chunks, &index_chunk_to_assets);

    self.minify_assets(&mut assets)?;

    let mut output = Vec::with_capacity(assets.len());
    let mut output_assets = vec![];
    for Asset {
      mut map,
      meta: rendered_chunk,
      content: mut code,
      file_dir,
      preliminary_filename,
      filename,
      ..
    } in assets
    {
      if let InstantiationKind::Ecma(ecma_meta) = rendered_chunk {
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

          if let Some(sourcemap) = &self.options.sourcemap {
            match sourcemap {
              SourceMapType::File | SourceMapType::Hidden => {
                let source = map.to_json_string();
                output_assets.push(Output::Asset(Box::new(OutputAsset {
                  filename: map_filename.as_str().into(),
                  source: source.into(),
                  original_file_name: None,
                  name: None,
                })));
                if matches!(sourcemap, SourceMapType::File) {
                  code.push_str(&format!(
                    "\n//# sourceMappingURL={}",
                    Path::new(&map_filename)
                      .file_name()
                      .expect("should have filename")
                      .to_string_lossy()
                  ));
                }
              }
              SourceMapType::Inline => {
                let data_url = map.to_data_url();
                code.push_str(&format!("\n//# sourceMappingURL={data_url}"));
              }
            }
          }
        }

        let sourcemap_filename =
          if matches!(self.options.sourcemap, Some(SourceMapType::Inline) | None) {
            None
          } else {
            Some(format!("{}.map", rendered_chunk.filename.as_str()))
          };
        output.push(Output::Chunk(Box::new(OutputChunk {
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
      } else {
        output.push(Output::Asset(Box::new(OutputAsset {
          filename: filename.clone().into(),
          source: code.into(),
          original_file_name: None,
          name: None,
        })));
      }
    }

    // Make sure order of assets are deterministic
    // TODO: use `preliminary_filename` on `Output::Asset` instead
    output_assets.sort_unstable_by(|a, b| a.filename().cmp(b.filename()));

    // The chunks order make sure the entry chunk at first, the assets at last, see https://github.com/rollup/rollup/blob/master/src/rollup/rollup.ts#L266
    output.sort_unstable_by(|a, b| match (a, b) {
      (Output::Chunk(a), Output::Chunk(b)) => {
        if a.is_entry || b.is_entry {
          std::cmp::Ordering::Greater
        } else {
          a.filename.cmp(&b.filename)
        }
      }
      _ => std::cmp::Ordering::Equal,
    });

    output.extend(output_assets);

    Ok(BundleOutput { assets: output, errors, warnings })
  }

  async fn instantiate_chunks(
    &self,
    chunk_graph: &ChunkGraph,
    errors: &mut Vec<BuildDiagnostic>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> anyhow::Result<(IndexInstantiatedChunks, IndexChunkToAssets)> {
    let mut index_chunk_to_assets: IndexChunkToAssets =
      index_vec![IndexSet::default(); chunk_graph.chunk_table.len()];
    let mut index_preliminary_assets: IndexInstantiatedChunks =
      IndexVec::with_capacity(chunk_graph.chunk_table.len());
    let chunk_index_to_codegen_rets = self.create_chunk_to_codegen_ret_map(chunk_graph);

    try_join_all(
      chunk_graph.chunk_table.iter_enumerated().zip(chunk_index_to_codegen_rets.into_iter()).map(
        |((chunk_idx, chunk), module_id_to_codegen_ret)| async move {
          let mut ctx = GenerateContext {
            chunk_idx,
            chunk,
            options: self.options,
            link_output: self.link_output,
            chunk_graph,
            plugin_driver: self.plugin_driver,
            warnings: vec![],
            module_id_to_codegen_ret,
          };
          let ecma_chunks = EcmaGenerator::instantiate_chunk(&mut ctx).await;

          let mut ctx = GenerateContext {
            chunk_idx,
            chunk,
            options: self.options,
            link_output: self.link_output,
            chunk_graph,
            plugin_driver: self.plugin_driver,
            warnings: vec![],
            // FIXME: module_id_to_codegen_ret is currently not used in CssGenerator. But we need to pass it to satisfy the args.
            module_id_to_codegen_ret: vec![],
          };
          let css_chunks = CssGenerator::instantiate_chunk(&mut ctx).await;

          ecma_chunks.and_then(|ecma_chunks| css_chunks.map(|css_chunks| [ecma_chunks, css_chunks]))
        },
      ),
    )
    .await?
    .into_iter()
    .flatten()
    .for_each(|result| match result {
      Ok(generate_output) => {
        generate_output.chunks.into_iter().for_each(|asset| {
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

  /// Create a IndexVecMap from chunk index to related modules codegen return list.
  /// e.g.
  /// modules of chunk1: [ecma1, ecma2, external1]
  /// modules of chunk2: [ecma3, external2]
  /// ret: [
  ///   [Some(ecma1_codegen), Some(ecma2_codegen), None],
  ///   [Some(ecma3_codegen), None],
  /// ]
  fn create_chunk_to_codegen_ret_map(
    &self,
    chunk_graph: &ChunkGraph,
  ) -> Vec<Vec<Option<oxc::codegen::CodegenReturn>>> {
    let chunk_to_codegen_ret = chunk_graph
      .chunk_table
      .par_iter()
      .map(|item| {
        item
          .modules
          .par_iter()
          .map(|&module_idx| {
            if let Some(module) = self.link_output.module_table.modules[module_idx].as_normal() {
              let enable_sourcemap = self.options.sourcemap.is_some() && !module.is_virtual();

              // Because oxc codegen sourcemap is last of sourcemap chain,
              // If here no extra sourcemap need remapping, we using it as final module sourcemap.
              // So here make sure using correct `source_name` and `source_content.
              let render_output = EcmaCompiler::print(
                &self.link_output.ast_table[module.ecma_ast_idx()].0,
                &module.id,
                enable_sourcemap,
              );
              Some(render_output)
            } else {
              None
            }
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    chunk_to_codegen_ret
  }
}
