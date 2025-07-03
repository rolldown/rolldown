use std::{hash::Hash, mem};

use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{HashCharacters, InsChunkIdx, InstantiationKind, StrOrBytes};
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
  type_alias::{AssetVec, IndexChunkToInstances, IndexInstantiatedChunks},
};

#[allow(clippy::too_many_lines)]
#[tracing::instrument(level = "debug", skip_all)]
pub fn finalize_assets(
  chunk_graph: &ChunkGraph,
  link_output: &LinkStageOutput,
  index_instantiated_chunks: IndexInstantiatedChunks,
  index_chunk_to_instances: &IndexChunkToInstances,
  hash_characters: HashCharacters,
) -> AssetVec {
  let finder = hash_placeholder_left_finder();

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
      .map(|asset| match &asset.content {
        StrOrBytes::Str(content) => extract_hash_placeholders(content, &finder)
          .iter()
          .filter_map(|placeholder| ins_chunk_idx_by_placeholder.get(placeholder).copied())
          .collect_vec(),
        StrOrBytes::Bytes(_content) => {
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

  let index_ins_chunk_to_hashers: IndexVec<InsChunkIdx, Xxh3> =
    index_vec![Xxh3::default(); index_instantiated_chunks.len()];

  let index_final_hashes: IndexVec<InsChunkIdx, (String, u128)> = index_ins_chunk_to_hashers
    .into_par_iter()
    .enumerate()
    .map(|(asset_idx, mut hasher)| {
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
        &finder,
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

      // TODO: PERF: should check if this asset has dependencies/placeholders to be replaced
      match &mut instantiated_chunk.content {
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

      instantiated_chunk.finalize(filename)
    })
    .collect::<Vec<_>>();

  let index_ins_chunk_to_filename: IndexVec<InsChunkIdx, ArcStr> =
    assets.iter().map(|ins_chunk| ins_chunk.filename.clone()).collect::<Vec<_>>().into();

  assets.par_iter_mut().for_each(|ins_chunk| {
    if let InstantiationKind::Ecma(ecma_meta) = &mut ins_chunk.meta {
      let chunk = &chunk_graph.chunk_table[ins_chunk.originate_from];
      ecma_meta.imports = chunk
        .cross_chunk_imports
        .iter()
        .flat_map(|importee_idx| &index_chunk_to_instances[*importee_idx])
        .map(|importee_asset_idx| index_ins_chunk_to_filename[*importee_asset_idx].clone())
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
        .flat_map(|importee_idx| &index_chunk_to_instances[*importee_idx])
        .map(|importee_asset_idx| index_ins_chunk_to_filename[*importee_asset_idx].clone())
        .collect();
    }
  });

  assets
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
