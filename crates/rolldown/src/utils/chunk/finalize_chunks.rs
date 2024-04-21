use std::hash::{DefaultHasher, Hash, Hasher};

use index_vec::IndexVec;
use rolldown_common::ChunkId;
use rolldown_utils::{
  base64::to_url_safe_base64,
  rayon::{IntoParallelIterator, IntoParallelRefIterator, ParallelBridge, ParallelIterator},
  xxhash::xxhash_base64_url,
};
use rustc_hash::{FxHashMap, FxHashSet};
use xxhash_rust::xxh3::Xxh3;

use crate::{chunk_graph::ChunkGraph, utils::hash_placeholder::replace_facade_hash_replacement};

use super::render_chunk::ChunkRenderReturn;

pub fn finalize_chunks(
  chunk_graph: &mut ChunkGraph,
  mut chunks: Vec<ChunkRenderReturn>,
) -> Vec<ChunkRenderReturn> {
  fn append_hash(
    visited: &mut FxHashSet<ChunkId>,
    state: &mut impl Hasher,
    chunk_id: ChunkId,
    chunk_graph: &ChunkGraph,
    chunk_refs: &IndexVec<ChunkId, &ChunkRenderReturn>,
    index_standalone_content_hashes: &IndexVec<ChunkId, String>,
  ) {
    if visited.contains(&chunk_id) {
      return;
    }
    visited.insert(chunk_id);

    // perf: maybe we could reuse the `hash_states_of_chunks` directly rather than rehashing the full content
    index_standalone_content_hashes[chunk_id].hash(state);

    tracing::debug!(
      "append_hash {filename}, hash: {hash}",
      filename = chunk_refs[chunk_id].rendered_chunk.file_name,
      hash = format!("{:X}", state.finish())
    );

    for dep in &chunk_graph.chunks[chunk_id].cross_chunk_imports {
      append_hash(visited, state, *dep, chunk_graph, chunk_refs, index_standalone_content_hashes);
    }
  }

  let index_standalone_content_hashes: IndexVec<ChunkId, String> = chunks
    .par_iter()
    .map(|chunk| xxhash_base64_url(chunk.code.as_bytes()))
    .collect::<Vec<_>>()
    .into();

  let mut index_chunk_hashers = index_vec::index_vec![Xxh3::default(); chunks.len()];

  let chunk_refs = chunks.iter().collect::<IndexVec<ChunkId, _>>();

  let index_final_hashes: IndexVec<ChunkId, String> = index_chunk_hashers
    .iter_mut_enumerated()
    // FIXME: Extra traversing. This is a workaround due to `par_bridge` doesn't ensure order https://github.com/rayon-rs/rayon/issues/551#issuecomment-882069261
    .collect::<Vec<_>>()
    .into_par_iter()
    .map(|(chunk_id, state)| {
      let chunk_render_ret: &ChunkRenderReturn = chunk_refs[chunk_id];
      let tracing_span =
        tracing::debug_span!("append_hash", filename = chunk_render_ret.rendered_chunk.file_name);
      let _entered = tracing_span.enter();

      let mut visited = FxHashSet::default();
      append_hash(
        &mut visited,
        state,
        chunk_id,
        chunk_graph,
        &chunk_refs,
        &index_standalone_content_hashes,
      );
      let digested = state.digest128();
      tracing::debug!("digested: {digested}");
      to_url_safe_base64(digested.to_le_bytes())
    })
    .collect::<Vec<_>>()
    .into();

  tracing::debug!("index_final_hashes: {:#?}", index_final_hashes);

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

  tracing::debug!("final_hashes_by_placeholder: {:#?}", final_hashes_by_placeholder);

  chunk_graph.chunks.iter_mut().zip(chunks.iter_mut()).par_bridge().for_each(
    |(chunk, chunk_render_return)| {
      let preliminary_filename_raw =
        chunk.preliminary_filename.as_ref().expect("should have file name").to_string();
      let filename =
        replace_facade_hash_replacement(preliminary_filename_raw, &final_hashes_by_placeholder);
      chunk.filename = Some(filename.clone());
      chunk_render_return.rendered_chunk.file_name = filename;
      chunk_render_return.code = replace_facade_hash_replacement(
        std::mem::take(&mut chunk_render_return.code),
        &final_hashes_by_placeholder,
      );
    },
  );

  chunks
}
