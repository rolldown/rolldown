use rolldown_utils::block_on_spawn_all;

use crate::{
  bundler::{chunk::ChunkId, plugin_driver::PluginDriver},
  error::{collect_errors, BatchedErrors},
  plugin::args::RenderChunkArgs,
};

#[allow(clippy::unused_async)]
pub async fn _render_chunks<'a>(
  plugin_driver: &PluginDriver,
  chunks: Vec<(ChunkId, String)>,
) -> Result<Vec<(ChunkId, String)>, BatchedErrors> {
  let result = block_on_spawn_all(chunks.iter().map(|(chunk, content)| async move {
    match plugin_driver.render_chunk(RenderChunkArgs { code: content.to_string() }).await {
      Ok(value) => Ok((*chunk, value)),
      Err(e) => Err(e),
    }
  }));

  collect_errors(result)
}
