use std::path::Path;

use futures::future::try_join_all;
use oxc_index::{index_vec, IndexVec};
use rolldown_common::{
  Asset, InstantiationKind, ModuleRenderArgs, ModuleRenderOutput, Output, OutputAsset, OutputChunk,
  SourceMapType,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::{
  concat_string,
  indexmap::FxIndexSet,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};
use sugar_path::SugarPath;

use crate::{
  asset::asset_generator::AssetGenerator,
  chunk_graph::ChunkGraph,
  css::css_generator::CssGenerator,
  ecmascript::ecma_generator::EcmaGenerator,
  type_alias::{IndexChunkToAssets, IndexInstantiatedChunks},
  types::generator::{GenerateContext, Generator},
  utils::{
    augment_chunk_hash::augment_chunk_hash, chunk::finalize_chunks::finalize_assets,
    render_chunks::render_chunks, uuid::uuid_v4_string_from_u128,
  },
  BundleOutput,
};

use super::GenerateStage;

impl<'a> GenerateStage<'a> {
  #[allow(clippy::too_many_lines)]
  pub async fn render_chunk_to_assets(
    &mut self,
    chunk_graph: &mut ChunkGraph,
  ) -> BuildResult<BundleOutput> {
    let mut errors = std::mem::take(&mut self.link_output.errors);
    let mut warnings = std::mem::take(&mut self.link_output.warnings);
    let (mut instantiated_chunks, index_chunk_to_assets) =
      self.instantiate_chunks(chunk_graph, &mut errors, &mut warnings).await?;

    render_chunks(self.plugin_driver, &mut instantiated_chunks, self.options).await?;

    augment_chunk_hash(self.plugin_driver, &mut instantiated_chunks).await?;

    let mut assets = finalize_assets(
      chunk_graph,
      instantiated_chunks,
      &index_chunk_to_assets,
      self.options.hash_characters,
    );

    self.minify_assets(&mut assets)?;

    let mut output = Vec::with_capacity(assets.len());
    let mut output_assets = vec![];
    for Asset {
      mut map,
      meta: rendered_chunk,
      content: code,
      file_dir,
      preliminary_filename,
      filename,
      ..
    } in assets
    {
      if let InstantiationKind::Ecma(ecma_meta) = rendered_chunk {
        let mut code = code.try_into_string()?;
        let rendered_chunk = ecma_meta.rendered_chunk;
        if let Some(map) = map.as_mut() {
          let file_base_name =
            Path::new(rendered_chunk.filename.as_str()).file_name().expect("should have file name");
          map.set_file(file_base_name.to_string_lossy().as_ref());

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

          if self.options.sourcemap_debug_ids && self.options.sourcemap.is_some() {
            let debug_id_str = uuid_v4_string_from_u128(rendered_chunk.debug_id);
            map.set_debug_id(&debug_id_str);
            code.push_str("\n//# debugId=");
            code.push_str(debug_id_str.as_str());
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
                  original_file_names: vec![],
                  names: vec![],
                })));
                if matches!(sourcemap, SourceMapType::File) {
                  code.push_str("\n//# sourceMappingURL=");
                  code.push_str(
                    &Path::new(&map_filename)
                      .file_name()
                      .expect("should have filename")
                      .to_string_lossy(),
                  );
                }
              }
              SourceMapType::Inline => {
                let data_url = map.to_data_url();
                code.push_str("\n//# sourceMappingURL=");
                code.push_str(&data_url);
              }
            }
          }
        }

        let sourcemap_filename =
          if matches!(self.options.sourcemap, Some(SourceMapType::Inline) | None) {
            None
          } else {
            Some(concat_string!(rendered_chunk.filename, ".map"))
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
          source: code,
          original_file_names: vec![],
          names: vec![],
        })));
      }
    }

    // Make sure order of assets are deterministic
    // TODO: use `preliminary_filename` on `Output::Asset` instead
    output_assets.sort_unstable_by(|a, b| a.filename().cmp(b.filename()));

    // The chunks order make sure the entry chunk at first, the assets at last, see https://github.com/rollup/rollup/blob/master/src/rollup/rollup.ts#L266
    output.sort_unstable_by(|a, b| {
      let a_type = get_sorting_file_type(a) as u8;
      let b_type = get_sorting_file_type(b) as u8;
      if a_type == b_type {
        return a.filename().cmp(b.filename());
      }
      a_type.cmp(&b_type)
    });

    output.extend(output_assets);

    if !errors.is_empty() {
      return Err(errors.into());
    }

    Ok(BundleOutput {
      assets: output,
      warnings,
      watch_files: self.plugin_driver.watch_files.iter().map(|f| f.clone()).collect(),
    })
  }

  async fn instantiate_chunks(
    &self,
    chunk_graph: &ChunkGraph,
    errors: &mut Vec<BuildDiagnostic>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> BuildResult<(IndexInstantiatedChunks, IndexChunkToAssets)> {
    let mut index_chunk_to_assets: IndexChunkToAssets =
      index_vec![FxIndexSet::default(); chunk_graph.chunk_table.len()];
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

          let mut ctx = GenerateContext {
            chunk_idx,
            chunk,
            options: self.options,
            link_output: self.link_output,
            chunk_graph,
            plugin_driver: self.plugin_driver,
            warnings: vec![],
            // FIXME: module_id_to_codegen_ret is currently not used in AssetGenerator. But we need to pass it to satisfy the args.
            module_id_to_codegen_ret: vec![],
          };
          let asset_chunks = AssetGenerator::instantiate_chunk(&mut ctx).await;

          ecma_chunks.and_then(|ecma_chunks| {
            css_chunks.and_then(|css_chunks| {
              asset_chunks.map(|asset_chunks| [ecma_chunks, css_chunks, asset_chunks])
            })
          })
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
      Err(e) => errors.extend(e.into_vec()),
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
  ) -> Vec<Vec<Option<ModuleRenderOutput>>> {
    let chunk_to_codegen_ret = chunk_graph
      .chunk_table
      .par_iter()
      .map(|item| {
        item
          .modules
          .par_iter()
          .map(|&module_idx| {
            if let Some(module) = self.link_output.module_table.modules[module_idx].as_normal() {
              let ast = &self.link_output.ast_table[module.ecma_ast_idx()].0;
              module.render(self.options, &ModuleRenderArgs::Ecma { ast })
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

enum SortingFileType {
  EntryChunk = 0,
  SecondaryChunk = 1,
  Asset = 2,
}

#[inline]
fn get_sorting_file_type(output: &Output) -> SortingFileType {
  match output {
    Output::Asset(_) => SortingFileType::Asset,
    Output::Chunk(chunk) => {
      if chunk.is_entry {
        SortingFileType::EntryChunk
      } else {
        SortingFileType::SecondaryChunk
      }
    }
  }
}
