use rolldown_common::IntoBatchedResult;
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::collapse_sourcemaps;
use rolldown_utils::block_on_spawn_all;

use crate::{chunk::ChunkRenderReturn, error::BatchedErrors};

pub async fn render_chunks<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: Vec<ChunkRenderReturn>,
) -> Result<Vec<ChunkRenderReturn>, BatchedErrors> {
  let result = block_on_spawn_all(chunks.into_iter().map(|chunk| async move {
    tracing::info!("render_chunks");
    match plugin_driver
      .render_chunk(HookRenderChunkArgs { code: chunk.code, chunk: &chunk.rendered_chunk })
      .await
    {
      Ok((code, render_chunk_sourcemap_chain)) => Ok(ChunkRenderReturn {
        code,
        map: if render_chunk_sourcemap_chain.is_empty() {
          chunk.map
        } else {
          let mut sourcemap_chain = Vec::with_capacity(render_chunk_sourcemap_chain.len() + 1);
          if let Some(sourcemap) = chunk.map.as_ref() {
            sourcemap_chain.push(sourcemap);
          }
          sourcemap_chain.extend(render_chunk_sourcemap_chain.iter());
          collapse_sourcemaps(sourcemap_chain, None)
        },
        rendered_chunk: chunk.rendered_chunk,
      }),
      Err(e) => Err(e),
    }
  }));

  result.into_batched_result()
}
