use std::hash::Hash;

use itertools::Itertools;
use oxc::index::IndexVec;
use rolldown_common::{ChunkId, ResourceId};
use rolldown_utils::{
  base64::to_url_safe_base64,
  rayon::{IntoParallelIterator, IntoParallelRefIterator, ParallelBridge, ParallelIterator},
  xxhash::xxhash_base64_url,
};
use rustc_hash::FxHashMap;
use xxhash_rust::xxh3::Xxh3;

use crate::{
  chunk_graph::ChunkGraph,
  utils::hash_placeholder::{extract_hash_placeholders, replace_facade_hash_replacement},
};

use super::render_chunk::ChunkRenderReturn;

#[tracing::instrument(level = "debug", skip_all)]
pub fn finalize_chunks(
  chunk_graph: &mut ChunkGraph,
  mut chunks: Vec<ChunkRenderReturn>,
) -> Vec<ChunkRenderReturn> {
  let chunk_id_by_placeholder = chunk_graph
    .chunks
    .iter_enumerated()
    .filter_map(|(chunk_id, chunk)| {
      chunk
        .preliminary_filename
        .as_ref()
        .unwrap()
        .hash_placeholder()
        .map(|hash_placeholder| (hash_placeholder.to_string(), chunk_id))
    })
    .collect::<FxHashMap<_, _>>();

  let index_chunk_dependencies: IndexVec<ChunkId, Vec<ChunkId>> = chunks
    .par_iter()
    .map(|chunk| {
      extract_hash_placeholders(&chunk.code)
        .iter()
        .map(|placeholder| chunk_id_by_placeholder[placeholder])
        .collect_vec()
    })
    .collect::<Vec<_>>()
    .into();

  let index_standalone_content_hashes: IndexVec<ChunkId, String> = chunks
    .par_iter()
    .map(|chunk| {
      let mut content = chunk.code.as_bytes().to_vec();
      if let Some(augment_chunk_hash) = &chunk.augment_chunk_hash {
        content.extend(augment_chunk_hash.as_bytes());
      }
      xxhash_base64_url(&content)
    })
    .collect::<Vec<_>>()
    .into();

  let mut index_chunk_hashers: IndexVec<ChunkId, Xxh3> =
    oxc::index::index_vec![Xxh3::default(); chunks.len()];

  let index_final_hashes: IndexVec<ChunkId, String> = index_chunk_hashers
    .iter_mut_enumerated()
    // FIXME: Extra traversing. This is a workaround due to `par_bridge` doesn't ensure order https://github.com/rayon-rs/rayon/issues/551#issuecomment-882069261
    .collect::<Vec<_>>()
    .into_par_iter()
    .map(|(chunk_id, state)| {
      // hash itself
      index_standalone_content_hashes[chunk_id].hash(state);
      // hash itself's preliminary filename to prevent different chunks that have the same content from having the same hash
      chunk_graph.chunks[chunk_id]
        .preliminary_filename
        .as_ref()
        .expect("must have preliminary_filename")
        .hash(state);
      let dependencies = &index_chunk_dependencies[chunk_id];
      dependencies.iter().copied().for_each(|dep_id| {
        index_standalone_content_hashes[dep_id].hash(state);
      });
      let digested = state.digest128();
      to_url_safe_base64(digested.to_le_bytes())
    })
    .collect::<Vec<_>>()
    .into();

  let final_hashes_by_placeholder = chunk_graph
    .chunks
    .iter()
    .zip(&index_final_hashes)
    .filter_map(|(chunk, hash)| {
      chunk
        .preliminary_filename
        .as_ref()
        .unwrap()
        .hash_placeholder()
        .map(|hash_placeholder| (hash_placeholder.to_string(), &hash[..hash_placeholder.len()]))
    })
    .collect::<FxHashMap<_, _>>();

  chunk_graph.chunks.iter_mut().zip(chunks.iter_mut()).par_bridge().for_each(
    |(chunk, chunk_render_return)| {
      let preliminary_filename_raw =
        chunk.preliminary_filename.as_deref().expect("should have file name").to_string();
      let filename: ResourceId =
        replace_facade_hash_replacement(preliminary_filename_raw, &final_hashes_by_placeholder)
          .into();
      chunk.filename = Some(filename.clone());
      chunk_render_return.rendered_chunk.filename = filename;
      chunk_render_return.code = replace_facade_hash_replacement(
        std::mem::take(&mut chunk_render_return.code),
        &final_hashes_by_placeholder,
      );
    },
  );

  // Replace hash placeholder in `imports`
  chunk_graph.chunks.iter().zip(chunks.iter_mut()).par_bridge().for_each(
    |(chunk, chunk_render_return)| {
      chunk_render_return.rendered_chunk.imports = chunk
        .cross_chunk_imports
        .iter()
        .map(|id| chunk_graph.chunks[*id].filename.clone().expect("should have file name"))
        .collect();
      chunk_render_return.rendered_chunk.dynamic_imports = chunk
        .cross_chunk_dynamic_imports
        .iter()
        .map(|id| chunk_graph.chunks[*id].filename.clone().expect("should have file name"))
        .collect();
    },
  );

  chunks
}
