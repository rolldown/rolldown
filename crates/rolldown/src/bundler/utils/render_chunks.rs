use rolldown_utils::block_on_spawn_all;

use crate::{
  bundler::{chunk::render_chunk::RenderedChunk, plugin_driver::SharedPluginDriver},
  error::{into_batched_result, BatchedErrors},
  plugin::args::RenderChunkArgs,
};

#[allow(clippy::future_not_send)]
pub async fn render_chunks<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: impl Iterator<Item = (String, RenderedChunk)>,
) -> Result<Vec<(String, RenderedChunk)>, BatchedErrors> {
  let result = block_on_spawn_all(chunks.map(|(content, rendered_chunk)| async move {
    match plugin_driver
      .render_chunk(RenderChunkArgs {
        code: content,
        chunk: unsafe { std::mem::transmute(&rendered_chunk) },
      })
      .await
    {
      Ok(value) => Ok((value, rendered_chunk)),
      Err(e) => Err(e),
    }
  }));

  into_batched_result(result)
}
