use std::{ops::Deref, sync::Arc};

use futures::future::try_join_all;
use oxc::span::CompactStr;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Asset, ChunkIdx, EmittedChunkInfo, InstantiationKind, ModuleRenderArgs, ModuleRenderOutput,
  Output, OutputAsset, OutputChunk, SharedFileEmitter, SymbolRef,
};
use rolldown_debug::{action, trace_action, trace_action_enabled};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::{
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::{IntoParallelRefIterator, ParallelIterator},
};

use crate::{
  BundleOutput,
  asset::asset_generator::AssetGenerator,
  chunk_graph::ChunkGraph,
  css::css_generator::CssGenerator,
  ecmascript::ecma_generator::EcmaGenerator,
  type_alias::{AssetVec, IndexChunkToInstances, IndexInstantiatedChunks},
  types::generator::{GenerateContext, Generator},
  utils::{
    augment_chunk_hash::augment_chunk_hash,
    chunk::{finalize_chunks::finalize_assets, render_chunk_exports::get_export_items},
    render_chunks::render_chunks,
  },
};

use super::GenerateStage;

impl GenerateStage<'_> {
  #[allow(clippy::too_many_lines)]
  pub async fn render_chunk_to_assets(
    &mut self,
    chunk_graph: &ChunkGraph,
  ) -> BuildResult<BundleOutput> {
    let mut errors = std::mem::take(&mut self.link_output.errors);
    let mut warnings = std::mem::take(&mut self.link_output.warnings);
    let (mut instantiated_chunks, index_chunk_to_instances) =
      self.instantiate_chunks(chunk_graph, &mut errors, &mut warnings).await?;

    render_chunks(self.plugin_driver, &mut instantiated_chunks, self.options).await?;

    augment_chunk_hash(self.plugin_driver, &mut instantiated_chunks).await?;

    let assets = finalize_assets(
      chunk_graph,
      self.link_output,
      instantiated_chunks,
      &index_chunk_to_instances,
      self.options.hash_characters,
      self.options,
    )
    .await?;

    // Set emitted chunk info for file emitter, it should be set before call generate_bundle hook
    set_emitted_chunk_filenames(&self.plugin_driver.file_emitter, &assets, chunk_graph);

    Self::trace_action_assets_ready(&assets);

    let mut output = Vec::with_capacity(assets.len());
    let mut output_assets: Vec<Output> = vec![];
    for Asset { map, meta: rendered_chunk, content: code, filename, .. } in assets {
      match rendered_chunk {
        InstantiationKind::Ecma(ecma_meta) => {
          let code = code.try_into_string()?;
          let rendered_chunk = ecma_meta.rendered_chunk;
          output.push(Output::Chunk(Arc::new(OutputChunk {
            name: rendered_chunk.name.clone(),
            filename: filename.clone(),
            code,
            is_entry: rendered_chunk.is_entry,
            is_dynamic_entry: rendered_chunk.is_dynamic_entry,
            facade_module_id: rendered_chunk.facade_module_id.clone(),
            modules: rendered_chunk.modules.clone(),
            exports: rendered_chunk.exports.clone(),
            module_ids: rendered_chunk.module_ids.clone(),
            imports: ecma_meta.imports,
            dynamic_imports: ecma_meta.dynamic_imports,
            map,
            sourcemap_filename: ecma_meta.sourcemap_filename,
            preliminary_filename: ecma_meta.preliminary_filename.to_string(),
          })));
        }
        InstantiationKind::Css(_css_meta) => {
          let code = code.try_into_string()?;
          output.push(Output::Asset(Arc::new(OutputAsset {
            filename: filename.clone(),
            source: code.into(),
            original_file_names: vec![],
            names: vec![],
          })));
        }
        InstantiationKind::Sourcemap(sourcemap_meta) => {
          output.push(Output::Asset(Arc::new(OutputAsset {
            filename: filename.clone(),
            source: code,
            original_file_names: sourcemap_meta.original_file_names,
            names: sourcemap_meta.names,
          })));
        }
        InstantiationKind::None => {
          output.push(Output::Asset(Arc::new(OutputAsset {
            filename: filename.clone(),
            source: code,
            original_file_names: vec![],
            names: vec![],
          })));
        }
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

    Ok(BundleOutput { assets: output, warnings })
  }

  async fn instantiate_chunks(
    &self,
    chunk_graph: &ChunkGraph,
    errors: &mut Vec<BuildDiagnostic>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> BuildResult<(IndexInstantiatedChunks, IndexChunkToInstances)> {
    let mut index_chunk_to_instances: IndexChunkToInstances =
      index_vec![FxIndexSet::default(); chunk_graph.chunk_table.len()];
    let mut index_instantiated_chunks: IndexInstantiatedChunks =
      IndexVec::with_capacity(chunk_graph.chunk_table.len());
    let chunk_index_to_codegen_rets = self.create_chunk_to_codegen_ret_map(chunk_graph);
    let render_export_items_index_vec = &chunk_graph
      .chunk_table
      .chunks
      .iter()
      .map(|item| {
        let mut map: FxIndexMap<SymbolRef, Vec<CompactStr>> = FxIndexMap::default();
        get_export_items(item).into_iter().for_each(|(k, v)| {
          map.entry(v).or_default().push(k);
        });
        map
      })
      .collect();

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
            render_export_items_index_vec,
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
            render_export_items_index_vec: &index_vec![],
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
            render_export_items_index_vec: &index_vec![],
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
        generate_output.chunks.into_iter().for_each(|ins_chunk| {
          let base_chunk_idx = ins_chunk.originate_from;
          let ins_chunk_idx = index_instantiated_chunks.push(ins_chunk);
          index_chunk_to_instances[base_chunk_idx].insert(ins_chunk_idx);
        });
        warnings.extend(generate_output.warnings);
      }
      Err(e) => errors.extend(e.into_vec()),
    });

    index_chunk_to_instances.iter_mut().for_each(|instances| {
      instances.sort_by_cached_key(|ins_chunk_idx| {
        index_instantiated_chunks[*ins_chunk_idx].preliminary_filename.as_str()
      });
    });

    Ok((index_instantiated_chunks, index_chunk_to_instances))
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
    chunk_graph
      .chunk_table
      .par_iter()
      .map(|item| {
        item
          .modules
          .par_iter()
          .map(|&module_idx| match self.link_output.module_table[module_idx].as_normal() {
            Some(module) => {
              let ast = self.link_output.ast_table[module.idx].as_ref().expect("should have ast");
              module.render(self.options, &ModuleRenderArgs::Ecma { ast })
            }
            _ => None,
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>()
  }

  fn trace_action_assets_ready(index_assets: &AssetVec) {
    if trace_action_enabled!() {
      let mut assets = vec![];
      for asset in index_assets {
        assets.push(action::Asset {
          chunk_id: asset.originate_from.map(ChunkIdx::raw),
          content: asset.content.try_as_inner_str().ok().map(str::to_string),
          size: asset.content.as_bytes().len().try_into().unwrap(),
          filename: asset.filename.to_string(),
        });
      }
      let assets_ready = action::AssetsReady { action: "AssetsReady", assets };
      trace_action!(assets_ready);
    }
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

pub fn set_emitted_chunk_preliminary_filenames(
  file_emitter: &SharedFileEmitter,
  chunk_graph: &ChunkGraph,
) {
  let emitted_chunk_info = chunk_graph
    .chunk_table
    .chunks
    .iter_enumerated()
    .filter_map(|(idx, chunk)| {
      chunk_graph.chunk_idx_to_reference_ids.get(&idx).map(|reference_ids| {
        reference_ids.iter().map(|reference_id| EmittedChunkInfo {
          reference_id: reference_id.clone(),
          filename: chunk
            .preliminary_filename
            .as_ref()
            .expect("Emitted chunk should have filename")
            .deref()
            .clone(),
        })
      })
    })
    .flatten();
  file_emitter.set_emitted_chunk_info(emitted_chunk_info);
}

fn set_emitted_chunk_filenames(
  file_emitter: &SharedFileEmitter,
  assets: &AssetVec,
  chunk_graph: &ChunkGraph,
) {
  let emitted_chunk_info = assets
    .iter()
    .filter_map(|asset| {
      asset.originate_from.and_then(|originate_from| {
        chunk_graph.chunk_idx_to_reference_ids.get(&originate_from).map(|reference_ids| {
          reference_ids.iter().map(|reference_id| EmittedChunkInfo {
            reference_id: reference_id.clone(),
            filename: asset.filename.clone(),
          })
        })
      })
    })
    .flatten();
  file_emitter.set_emitted_chunk_info(emitted_chunk_info);
}
