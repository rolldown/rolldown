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
    let mut sourcemap_chain = vec![];
    if let Some(sourcemap) = chunk.map {
      sourcemap_chain.push(sourcemap);
    }

    match plugin_driver
      .render_chunk(
        HookRenderChunkArgs { code: chunk.code, chunk: &chunk.rendered_chunk },
        &mut sourcemap_chain,
      )
      .await
    {
      Ok(code) => Ok(ChunkRenderReturn {
        code,
        map: collapse_sourcemaps(sourcemap_chain, None),
        rendered_chunk: chunk.rendered_chunk,
      }),
      Err(e) => Err(e),
    }
  }));

  result.into_batched_result()
}
