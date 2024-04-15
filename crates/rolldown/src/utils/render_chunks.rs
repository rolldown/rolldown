use super::chunk::render_chunk::ChunkRenderReturn;
use anyhow::Result;
use futures::future::try_join_all;
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::collapse_sourcemaps;

pub async fn render_chunks<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: Vec<ChunkRenderReturn>,
) -> Result<Vec<ChunkRenderReturn>> {
  try_join_all(chunks.into_iter().map(|chunk| async move {
    tracing::info!("render_chunks");
    plugin_driver
      .render_chunk(HookRenderChunkArgs { code: chunk.code, chunk: &chunk.rendered_chunk })
      .await
      .map(|(code, render_chunk_sourcemap_chain)| ChunkRenderReturn {
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
      })
  }))
  .await
}
