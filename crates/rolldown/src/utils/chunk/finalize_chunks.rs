use std::{hash::Hash, mem};

use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{AssetIdx, HashCharacters, InstantiationKind, StrOrBytes};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  hash_placeholder::{
    extract_hash_placeholders, hash_placeholder_left_finder, replace_placeholder_with_hash,
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
  stages::link_stage::LinkStageOutput,
  type_alias::{IndexAssets, IndexChunkToAssets, IndexInstantiatedChunks},
};

#[tracing::instrument(level = "debug", skip_all)]
pub fn finalize_assets(
  chunk_graph: &mut ChunkGraph,
  link_output: &LinkStageOutput,
  preliminary_assets: IndexInstantiatedChunks,
  index_chunk_to_assets: &IndexChunkToAssets,
  hash_characters: HashCharacters,
) -> IndexAssets {
  let finder = hash_placeholder_left_finder();

  let asset_idx_by_placeholder = preliminary_assets
    .iter_enumerated()
    .filter_map(|(asset_idx, asset)| {
      asset.preliminary_filename.hash_placeholder().map(move |placeholders| {
        placeholders.iter().map(move |hash_placeholder| (hash_placeholder.as_str(), asset_idx))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let index_direct_dependencies: IndexVec<AssetIdx, Vec<AssetIdx>> = preliminary_assets
    .par_iter()
    .map(|asset| match &asset.content {
      StrOrBytes::Str(content) => extract_hash_placeholders(content, &finder)
        .iter()
        .filter_map(|placeholder| asset_idx_by_placeholder.get(placeholder).copied())
        .collect_vec(),
      StrOrBytes::Bytes(_content) => {
        vec![]
      }
    })
    .collect::<Vec<_>>()
    .into();

  // Instead of using `index_direct_dependencies`, we are gonna use `index_transitive_dependencies` to calculate the hash.
  // The reason is that we want to make sure, in `a -> b -> c`, if `c` is changed, not only the direct dependency `b` is changed, but also the indirect dependency `a` is changed.
  let index_transitive_dependencies: IndexVec<AssetIdx, FxIndexSet<AssetIdx>> =
    collect_transitive_dependencies(&index_direct_dependencies);

  let hash_base = hash_characters.base();
  let index_standalone_content_hashes: IndexVec<AssetIdx, String> = preliminary_assets
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

  let index_asset_hashers: IndexVec<AssetIdx, Xxh3> =
    index_vec![Xxh3::default(); preliminary_assets.len()];

  let index_final_hashes: IndexVec<AssetIdx, (String, u128)> = index_asset_hashers
    .into_par_iter()
    .enumerate()
    .map(|(asset_idx, mut hasher)| {
      let asset_idx = AssetIdx::from(asset_idx);
      // Start to calculate hash, first we hash itself
      index_standalone_content_hashes[asset_idx].hash(&mut hasher);

      // hash itself's preliminary filename to prevent different chunks that have the same content from having the same hash
      preliminary_assets[asset_idx].preliminary_filename.hash(&mut hasher);

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
      preliminary_assets[idx].preliminary_filename.hash_placeholder().map(|placeholders| {
        placeholders.iter().map(|placeholder| (placeholder.clone(), &hash[..placeholder.len()]))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let mut assets: IndexAssets = preliminary_assets
    .into_par_iter()
    .enumerate()
    .map(|(asset_idx, mut asset)| {
      let asset_idx = AssetIdx::from(asset_idx);

      let filename: ArcStr = replace_placeholder_with_hash(
        asset.preliminary_filename.as_str(),
        &final_hashes_by_placeholder,
        &finder,
      )
      .into();

      if let InstantiationKind::Ecma(ecma_meta) = &mut asset.kind {
        let (_, debug_id) = index_final_hashes[asset_idx];
        ecma_meta.debug_id = debug_id;
      }
      if let InstantiationKind::Css(css_meta) = &mut asset.kind {
        css_meta.filename = filename.clone();
        let (_, debug_id) = index_final_hashes[asset_idx];
        css_meta.debug_id = debug_id;
      }

      // TODO: PERF: should check if this asset has dependencies/placeholders to be replaced
      match &mut asset.content {
        StrOrBytes::Str(content) => {
          *content = replace_placeholder_with_hash(
            &mem::take(content),
            &final_hashes_by_placeholder,
            &finder,
          )
          .into_owned();
        }
        StrOrBytes::Bytes(_content) => {}
      }

      asset.finalize(filename)
    })
    .collect::<Vec<_>>()
    .into();

  let index_asset_to_filename: IndexVec<AssetIdx, ArcStr> =
    assets.iter().map(|asset| asset.filename.clone()).collect::<Vec<_>>().into();

  assets.par_iter_mut().for_each(|asset| {
    if let InstantiationKind::Ecma(ecma_meta) = &mut asset.meta {
      let chunk = &chunk_graph.chunk_table[asset.origin_chunk];
      ecma_meta.imports = chunk
        .cross_chunk_imports
        .iter()
        .flat_map(|importee_idx| &index_chunk_to_assets[*importee_idx])
        .map(|importee_asset_idx| index_asset_to_filename[*importee_asset_idx].clone())
        .chain(
          chunk
            .imports_from_external_modules
            .iter()
            .map(|(idx, _)| link_output.module_table[*idx].id().into()),
        )
        .collect();

      ecma_meta.dynamic_imports = chunk
        .cross_chunk_dynamic_imports
        .iter()
        .flat_map(|importee_idx| &index_chunk_to_assets[*importee_idx])
        .map(|importee_asset_idx| index_asset_to_filename[*importee_asset_idx].clone())
        .collect();
    }
  });

  assets
}

fn collect_transitive_dependencies(
  index_direct_dependencies: &IndexVec<AssetIdx, Vec<AssetIdx>>,
) -> IndexVec<AssetIdx, FxIndexSet<AssetIdx>> {
  fn traverse(
    index: AssetIdx,
    dep_map: &IndexVec<AssetIdx, Vec<AssetIdx>>,
    visited: &mut FxIndexSet<AssetIdx>,
  ) {
    for dep_index in &dep_map[index] {
      if !visited.contains(dep_index) {
        visited.insert(*dep_index);
        traverse(*dep_index, dep_map, visited);
      }
    }
  }

  let index_transitive_dependencies: IndexVec<AssetIdx, FxIndexSet<AssetIdx>> =
    index_direct_dependencies
      .par_iter()
      .enumerate()
      .map(|(idx, _deps)| {
        let idx = AssetIdx::from(idx);
        let mut visited_deps = FxIndexSet::default();
        traverse(idx, index_direct_dependencies, &mut visited_deps);
        visited_deps
      })
      .collect::<Vec<_>>()
      .into();

  index_transitive_dependencies
}
