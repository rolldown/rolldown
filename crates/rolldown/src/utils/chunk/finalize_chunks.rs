use std::hash::{DefaultHasher, Hash, Hasher};

use index_vec::IndexVec;
use rolldown_common::ChunkId;
use rolldown_utils::rayon::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{chunk_graph::ChunkGraph, utils::hash_placeholder::replace_facade_hash_replacement};

use super::render_chunk::ChunkRenderReturn;

pub fn finalize_chunks(
  chunk_graph: &mut ChunkGraph,
  mut chunks: Vec<ChunkRenderReturn>,
) -> Vec<ChunkRenderReturn> {
  fn append_hash(
    visited: &mut FxHashSet<ChunkId>,
    state: &mut DefaultHasher,
    chunk_id: ChunkId,
    chunk_graph: &ChunkGraph,
    chunk_refs: &IndexVec<ChunkId, &ChunkRenderReturn>,
  ) {
    if visited.contains(&chunk_id) {
      return;
    }
    visited.insert(chunk_id);

    // perf: maybe we could reuse the `hash_states_of_chunks` directly rather than rehashing the full content
    chunk_refs[chunk_id].code.hash(state);

    tracing::debug!(
      "append_hash {filename}, hash: {hash}",
      filename = chunk_refs[chunk_id].rendered_chunk.file_name,
      hash = format!("{:X}", state.finish())
    );

    for dep in &chunk_graph.chunks[chunk_id].cross_chunk_imports {
      append_hash(visited, state, *dep, chunk_graph, chunk_refs);
    }
  }

  let mut hash_states_of_chunks = chunks
    .iter()
    .map(|_| {
      // TODO: use a better hash function
      DefaultHasher::default()
    })
    .collect::<IndexVec<ChunkId, _>>();

  let chunk_refs = chunks.iter().collect::<IndexVec<ChunkId, _>>();

  let finalized_hashes = hash_states_of_chunks
    .iter_mut_enumerated()
    // FIXME: Extra traversing. This is a workaround due to `par_bridge` doesn't ensure order https://github.com/rayon-rs/rayon/issues/551#issuecomment-882069261
    .collect::<Vec<_>>()
    .into_par_iter()
    .map(|(chunk_id, state)| {
      let tracing_span = tracing::debug_span!(
        "append_hash",
        filename = chunk_refs[chunk_id].rendered_chunk.file_name
      );
      let _entered = tracing_span.enter();

      let mut visited = FxHashSet::default();
      append_hash(&mut visited, state, chunk_id, chunk_graph, &chunk_refs);
      let hashed = format!("{:08X}", state.finish());
      tracing::debug!("hashed: {hashed}");
      hashed
    })
    .collect::<Vec<_>>();

  tracing::debug!("finalized_hashes: {:#?}", finalized_hashes);

  let placeholder_to_finalized_hashes = chunk_graph
    .chunks
    .iter()
    .zip(&finalized_hashes)
    .filter_map(|(chunk, hash)| {
      chunk
        .preliminary_filename
        .as_ref()
        .unwrap()
        .hash_placeholder()
        .map(|hash_placeholder| (hash_placeholder.to_string(), hash.as_str()))
    })
    .collect::<FxHashMap<_, _>>();

  tracing::debug!("placeholder_to_finalized_hashes: {:#?}", placeholder_to_finalized_hashes);

  chunk_graph.chunks.iter_mut().zip(chunks.iter_mut()).for_each(|(chunk, chunk_render_return)| {
    let preliminary_filename_raw =
      chunk.preliminary_filename.as_ref().expect("should have file name").to_string();
    let filename =
      replace_facade_hash_replacement(preliminary_filename_raw, &placeholder_to_finalized_hashes);
    chunk.filename = Some(filename.clone());
    chunk_render_return.rendered_chunk.file_name = filename;
    // TODO replace code
    // chunk_render_return.code = chunk_render_return.code.replace(from, to)
  });

  chunks.iter_mut().par_bridge().for_each(|chunk_ret| {
    chunk_ret.code = replace_facade_hash_replacement(
      std::mem::take(&mut chunk_ret.code),
      &placeholder_to_finalized_hashes,
    );
  });
  chunks
}
