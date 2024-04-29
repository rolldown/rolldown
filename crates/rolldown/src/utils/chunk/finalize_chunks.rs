use std::hash::{Hash, Hasher};

use index_vec::IndexVec;
use rolldown_common::{ChunkId, FilePath};
use rolldown_utils::{
  base64::to_url_safe_base64,
  rayon::{IntoParallelIterator, IntoParallelRefIterator, ParallelBridge, ParallelIterator},
  xxhash::xxhash_base64_url,
};
use rustc_hash::{FxHashMap, FxHashSet};
use xxhash_rust::xxh3::Xxh3;

use crate::{chunk_graph::ChunkGraph, utils::hash_placeholder::replace_facade_hash_replacement};

use super::render_chunk::ChunkRenderReturn;

#[tracing::instrument(level = "debug", skip_all)]
pub fn finalize_chunks(
  chunk_graph: &mut ChunkGraph,
  mut chunks: Vec<ChunkRenderReturn>,
) -> Vec<ChunkRenderReturn> {
  fn append_hash(
    visited: &mut FxHashSet<ChunkId>,
    state: &mut impl Hasher,
    chunk_id: ChunkId,
    chunk_graph: &ChunkGraph,
    index_standalone_content_hashes: &IndexVec<ChunkId, String>,
  ) {
    if visited.contains(&chunk_id) {
      return;
    }
    visited.insert(chunk_id);

    // perf: maybe we could reuse the `hash_states_of_chunks` directly rather than rehashing the full content
    index_standalone_content_hashes[chunk_id].hash(state);

    for dep in &chunk_graph.chunks[chunk_id].cross_chunk_imports {
      append_hash(visited, state, *dep, chunk_graph, index_standalone_content_hashes);
    }
  }

  let index_standalone_content_hashes: IndexVec<ChunkId, String> = chunks
    .par_iter()
    .map(|chunk| xxhash_base64_url(chunk.code.as_bytes()))
    .collect::<Vec<_>>()
    .into();

  let mut index_chunk_hashers = index_vec::index_vec![Xxh3::default(); chunks.len()];

  let index_final_hashes: IndexVec<ChunkId, String> = index_chunk_hashers
    .iter_mut_enumerated()
    // FIXME: Extra traversing. This is a workaround due to `par_bridge` doesn't ensure order https://github.com/rayon-rs/rayon/issues/551#issuecomment-882069261
    .collect::<Vec<_>>()
    .into_par_iter()
    .map(|(chunk_id, state)| {
      let mut visited = FxHashSet::default();
      append_hash(&mut visited, state, chunk_id, chunk_graph, &index_standalone_content_hashes);
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
      let filename: FilePath =
        replace_facade_hash_replacement(preliminary_filename_raw, &final_hashes_by_placeholder)
          .into();
      chunk.filename = Some(filename.clone());
      chunk_render_return.rendered_chunk.file_name = filename;
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
