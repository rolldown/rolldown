use super::chunk::render_chunk::ChunkRenderReturn;
use anyhow::Result;
use futures::future::try_join_all;
use rolldown_plugin::{HookBannerArgs, SharedPluginDriver};

#[tracing::instrument(level = "debug", skip_all)]
pub async fn banner<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: Vec<ChunkRenderReturn>,
) -> Result<Vec<ChunkRenderReturn>> {
  try_join_all(chunks.into_iter().map(|chunk| async move {
    plugin_driver.banner(HookBannerArgs { code: chunk.code }).await.map(|code| ChunkRenderReturn {
      code,
      map: chunk.map,
      rendered_chunk: chunk.rendered_chunk,
      augment_chunk_hash: chunk.augment_chunk_hash,
      file_dir: chunk.file_dir,
      preliminary_filename: chunk.preliminary_filename,
    })
  }))
  .await
}
