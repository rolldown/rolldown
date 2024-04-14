use std::hash::{DefaultHasher, Hash};

use index_vec::IndexVec;
use rolldown_common::{Chunk, ChunkId};
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rustc_hash::FxHashSet;

use crate::chunk_graph::ChunkGraph;

use super::render_chunk::ChunkRenderReturn;

pub fn finalize_chunks(
  chunk_graph: &mut ChunkGraph,
  chunks: Vec<ChunkRenderReturn>,
) -> Vec<ChunkRenderReturn> {
  let mut hash_states_of_chunks = chunks
    .iter()
    .map(|chunk| {
      // TODO: use a better hash function
      let mut state = DefaultHasher::default();
      chunk.code.hash(&mut state);
      state
    })
    .collect::<IndexVec<ChunkId, _>>();

  let chunk_refs = chunks.iter().collect::<IndexVec<ChunkId, _>>();

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

    // FIXME: not stable yet
    for dep in &chunk_graph.chunks[chunk_id].cross_chunk_imports {
      append_hash(visited, state, *dep, chunk_graph, chunk_refs);
    }
  }

  hash_states_of_chunks.iter_mut_enumerated().par_bridge().for_each(|(chunk, state)| {
    let mut visited = FxHashSet::default();
    append_hash(&mut visited, state, chunk, chunk_graph, &chunk_refs);
  });

  chunks
}
