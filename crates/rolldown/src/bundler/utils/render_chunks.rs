use rolldown_common::RenderedChunk;
use rolldown_plugin::RenderChunkArgs;
use rolldown_sourcemap::SourceMap;
use rolldown_utils::block_on_spawn_all;

use crate::{
  bundler::plugin_driver::SharedPluginDriver,
  error::{into_batched_result, BatchedErrors},
};

#[allow(clippy::future_not_send)]
pub async fn render_chunks<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: impl Iterator<Item = (String, Option<SourceMap>, RenderedChunk)>,
) -> Result<Vec<(String, Option<SourceMap>, RenderedChunk)>, BatchedErrors> {
  // TODO support `render_chunk` hook return map
  let result = block_on_spawn_all(chunks.map(|(content, map, rendered_chunk)| async move {
    match plugin_driver
      .render_chunk(RenderChunkArgs {
        code: content,
        chunk: unsafe { std::mem::transmute(&rendered_chunk) },
      })
      .await
    {
      Ok(value) => Ok((value, map, rendered_chunk)),
      Err(e) => Err(e),
    }
  }));

  into_batched_result(result)
}
