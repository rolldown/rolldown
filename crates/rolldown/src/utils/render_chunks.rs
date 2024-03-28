use rolldown_common::IntoBatchedResult;
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_utils::block_on_spawn_all;

use crate::{chunk::ChunkRenderReturn, error::BatchedErrors};

pub async fn render_chunks<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: Vec<ChunkRenderReturn>,
) -> Result<Vec<ChunkRenderReturn>, BatchedErrors> {
  // TODO support `render_chunk` hook return map
  let result = block_on_spawn_all(chunks.into_iter().map(|chunk| async move {
    tracing::info!("render_chunks");
    match plugin_driver
      .render_chunk(HookRenderChunkArgs { code: chunk.code, chunk: &chunk.rendered_chunk })
      .await
    {
      Ok(code) => {
        Ok(ChunkRenderReturn { code, map: chunk.map, rendered_chunk: chunk.rendered_chunk })
      }
      Err(e) => Err(e),
    }
  }));

  result.into_batched_result()
}
