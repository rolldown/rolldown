use rolldown_common::RenderedChunk;
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::block_on_spawn_all;

use crate::error::{BatchedErrors, IntoBatchedResult};

pub async fn render_chunks<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: impl Iterator<Item = (String, Option<SourceMap>, RenderedChunk)>,
) -> Result<Vec<(String, Option<SourceMap>, RenderedChunk)>, BatchedErrors> {
  // TODO support `render_chunk` hook return map
  let result = block_on_spawn_all(chunks.map(|(content, map, rendered_chunk)| async move {
    tracing::info!("render_chunks");
    match plugin_driver
      .render_chunk(HookRenderChunkArgs { code: content, chunk: &rendered_chunk })
      .await
    {
      Ok(value) => Ok((value, map, rendered_chunk)),
      Err(e) => Err(e),
    }
  }));

  result.into_batched_result()
}
