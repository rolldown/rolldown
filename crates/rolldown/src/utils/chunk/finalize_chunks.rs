use std::{borrow::Cow, hash::Hash, mem};

use arcstr::ArcStr;
use futures::future::try_join_all;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  Asset, HashCharacters, InsChunkIdx, InstantiationKind, NormalizedBundlerOptions, SourceMapType,
  StrOrBytes,
};
use rolldown_error::BuildResult;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  concat_string,
  hash_placeholder::{
    HASH_PLACEHOLDER_LEFT_FINDER, extract_hash_placeholders, replace_placeholder_with_hash,
  },
  indexmap::FxIndexSet,
  rayon::{
    IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
  },
  xxhash::{xxhash_base64_url, xxhash_with_base},
};
use rustc_hash::FxHashMap;
use xxhash_rust::xxh3::Xxh3;

use crate::{
  chunk_graph::ChunkGraph,
  stages::{generate_stage::GenerateStage, link_stage::LinkStageOutput},
  type_alias::{AssetVec, IndexChunkToInstances, IndexInstantiatedChunks},
  utils::process_code_and_sourcemap::process_code_and_sourcemap,
};

#[tracing::instrument(level = "debug", skip_all)]
pub async fn finalize_assets(
  chunk_graph: &ChunkGraph,
  link_output: &LinkStageOutput,
  index_instantiated_chunks: IndexInstantiatedChunks,
  index_chunk_to_instances: &IndexChunkToInstances,
  hash_characters: HashCharacters,
  options: &NormalizedBundlerOptions,
) -> BuildResult<AssetVec> {
  let ins_chunk_idx_by_placeholder = index_instantiated_chunks
    .iter_enumerated()
    .filter_map(|(ins_chunk_idx, ins_chunk)| {
      ins_chunk.preliminary_filename.hash_placeholder().map(move |placeholders| {
        placeholders.iter().map(move |hash_placeholder| (hash_placeholder.as_str(), ins_chunk_idx))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let index_direct_dependencies: IndexVec<InsChunkIdx, Vec<InsChunkIdx>> =
    index_instantiated_chunks
      .par_iter()
      .map(|asset| {
        if let StrOrBytes::Str(content) = &asset.content {
          extract_hash_placeholders(content, &HASH_PLACEHOLDER_LEFT_FINDER)
            .iter()
            .filter_map(|placeholder| ins_chunk_idx_by_placeholder.get(placeholder).copied())
            .collect_vec()
        } else {
          vec![]
        }
      })
      .collect::<Vec<_>>()
      .into();

  // Instead of using `index_direct_dependencies`, we are gonna use `index_transitive_dependencies` to calculate the hash.
  // The reason is that we want to make sure, in `a -> b -> c`, if `c` is changed, not only the direct dependency `b` is changed, but also the indirect dependency `a` is changed.
  let index_transitive_dependencies: IndexVec<InsChunkIdx, FxIndexSet<InsChunkIdx>> =
    collect_transitive_dependencies(&index_direct_dependencies);

  let hash_base = hash_characters.base();
  let index_standalone_content_hashes: IndexVec<InsChunkIdx, String> = index_instantiated_chunks
    .par_iter()
    .map(|chunk| {
      let mut hash = xxhash_base64_url(chunk.content.as_bytes());
      // Hash content that provided by users if it's exist
      if let Some(augment_chunk_hash) = &chunk.augment_chunk_hash {
        hash.push_str(augment_chunk_hash);
        hash = xxhash_base64_url(hash.as_bytes());
      }
      hash
    })
    .collect::<Vec<_>>()
    .into();

  let index_final_hashes: IndexVec<InsChunkIdx, (String, u128)> = (0..index_instantiated_chunks
    .len())
    .into_par_iter()
    .map(|asset_idx| {
      let mut hasher = Xxh3::default();
      let asset_idx = InsChunkIdx::from(asset_idx);
      // Start to calculate hash, first we hash itself
      index_standalone_content_hashes[asset_idx].hash(&mut hasher);

      // hash itself's preliminary filename to prevent different chunks that have the same content from having the same hash
      index_instantiated_chunks[asset_idx].preliminary_filename.hash(&mut hasher);

      let dependencies = &index_transitive_dependencies[asset_idx];
      dependencies.iter().copied().for_each(|dep_id| {
        index_standalone_content_hashes[dep_id].hash(&mut hasher);
      });

      let digested = hasher.digest128();
      (xxhash_with_base(&digested.to_le_bytes(), hash_base), digested)
    })
    .collect::<Vec<_>>()
    .into();

  let final_hashes_by_placeholder = index_final_hashes
    .iter_enumerated()
    .filter_map(|(idx, (hash, _))| {
      index_instantiated_chunks[idx].preliminary_filename.hash_placeholder().map(|placeholders| {
        placeholders.iter().map(|placeholder| (placeholder.clone(), &hash[..placeholder.len()]))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let mut assets: AssetVec = index_instantiated_chunks
    .into_par_iter()
    .enumerate()
    .map(|(asset_idx, mut instantiated_chunk)| {
      let asset_idx = InsChunkIdx::from(asset_idx);

      let filename: ArcStr = replace_placeholder_with_hash(
        instantiated_chunk.preliminary_filename.as_str(),
        &final_hashes_by_placeholder,
        &HASH_PLACEHOLDER_LEFT_FINDER,
      )
      .into();

      if let InstantiationKind::Ecma(ecma_meta) = &mut instantiated_chunk.kind {
        let (_, debug_id) = index_final_hashes[asset_idx];
        ecma_meta.debug_id = debug_id;
      }
      if let InstantiationKind::Css(css_meta) = &mut instantiated_chunk.kind {
        css_meta.filename = filename.clone();
        let (_, debug_id) = index_final_hashes[asset_idx];
        css_meta.debug_id = debug_id;
      }

      if let StrOrBytes::Str(content) = &mut instantiated_chunk.content {
        if let Cow::Owned(replaced) = replace_placeholder_with_hash(
          content,
          &final_hashes_by_placeholder,
          &HASH_PLACEHOLDER_LEFT_FINDER,
        ) {
          *content = replaced;
        }
      }

      instantiated_chunk.finalize(filename)
    })
    .collect::<Vec<_>>();

  let index_ins_chunk_to_filename: IndexVec<InsChunkIdx, ArcStr> =
    assets.iter().map(|ins_chunk| ins_chunk.filename.clone()).collect::<Vec<_>>().into();

  assets.par_iter_mut().for_each(|ins_chunk| {
    if let (InstantiationKind::Ecma(ecma_meta), Some(originate_from)) =
      (&mut ins_chunk.meta, ins_chunk.originate_from)
    {
      let chunk = &chunk_graph.chunk_table[originate_from];
      ecma_meta.imports = chunk
        .cross_chunk_imports
        .iter()
        .flat_map(|importee_idx| &index_chunk_to_instances[*importee_idx])
        .map(|importee_asset_idx| index_ins_chunk_to_filename[*importee_asset_idx].clone())
        .chain(
          chunk
            .direct_imports_from_external_modules
            .iter()
            .map(|(idx, _)| link_output.module_table[*idx].id().into()),
        )
        .collect();

      ecma_meta.dynamic_imports = chunk
        .cross_chunk_dynamic_imports
        .iter()
        .flat_map(|importee_idx| &index_chunk_to_instances[*importee_idx])
        .map(|importee_asset_idx| index_ins_chunk_to_filename[*importee_asset_idx].clone())
        .collect();
    }
  });

  GenerateStage::minify_assets(options, &mut assets)?;

  // apply sourcemap related logic

  let derived_assets = try_join_all(assets.iter_mut().map(async |asset| {
    let mut derived_asset: Result<Option<Asset>, anyhow::Error> = Ok(None::<Asset>);
    match &mut asset.meta {
      InstantiationKind::Ecma(ecma_meta) => {
        let asset_code = mem::take(&mut asset.content);
        let mut code = asset_code.try_into_string()?;
        if let Some(map) = asset.map.as_mut() {
          if let Some(sourcemap_asset) = process_code_and_sourcemap(
            options,
            &mut code,
            map,
            &ecma_meta.file_dir,
            asset.filename.as_str(),
            ecma_meta.debug_id,
            /*is_css*/ false,
          )
          .await?
          {
            derived_asset = Ok(Some(Asset {
              originate_from: None,
              content: sourcemap_asset.source,
              filename: sourcemap_asset.filename.clone(),
              map: None,
              meta: InstantiationKind::Sourcemap(Box::new(rolldown_common::SourcemapAssetMeta {
                names: sourcemap_asset.names,
                original_file_names: sourcemap_asset.original_file_names,
              })),
            }));
          }
        }

        let sourcemap_filename = if matches!(options.sourcemap, Some(SourceMapType::Inline) | None)
        {
          None
        } else {
          Some(concat_string!(asset.filename, ".map"))
        };
        ecma_meta.sourcemap_filename = sourcemap_filename;
        asset.content = code.into();
      }
      InstantiationKind::Css(css_meta) => {
        let asset_code = mem::take(&mut asset.content);
        let mut code = asset_code.try_into_string()?;
        if let Some(map) = asset.map.as_mut() {
          if let Some(sourcemap_asset) = process_code_and_sourcemap(
            options,
            &mut code,
            map,
            &css_meta.file_dir,
            asset.filename.as_str(),
            css_meta.debug_id,
            /*is_css*/ true,
          )
          .await?
          {
            derived_asset = Ok(Some(Asset {
              originate_from: None,
              content: sourcemap_asset.source,
              filename: sourcemap_asset.filename,
              map: None,
              meta: InstantiationKind::None,
            }));
          }
        }

        asset.content = code.into();
      }
      InstantiationKind::None | InstantiationKind::Sourcemap(_) => {}
    }
    derived_asset
  }))
  .await?;

  assets.extend(derived_assets.into_iter().flatten());

  Ok(assets)
}

fn collect_transitive_dependencies(
  index_direct_dependencies: &IndexVec<InsChunkIdx, Vec<InsChunkIdx>>,
) -> IndexVec<InsChunkIdx, FxIndexSet<InsChunkIdx>> {
  fn traverse(
    index: InsChunkIdx,
    dep_map: &IndexVec<InsChunkIdx, Vec<InsChunkIdx>>,
    visited: &mut FxIndexSet<InsChunkIdx>,
  ) {
    for dep_index in &dep_map[index] {
      if !visited.contains(dep_index) {
        visited.insert(*dep_index);
        traverse(*dep_index, dep_map, visited);
      }
    }
  }

  let index_transitive_dependencies: IndexVec<InsChunkIdx, FxIndexSet<InsChunkIdx>> =
    index_direct_dependencies
      .par_iter()
      .enumerate()
      .map(|(idx, _deps)| {
        let idx = InsChunkIdx::from(idx);
        let mut visited_deps = FxIndexSet::default();
        traverse(idx, index_direct_dependencies, &mut visited_deps);
        visited_deps
      })
      .collect::<Vec<_>>()
      .into();

  index_transitive_dependencies
}
