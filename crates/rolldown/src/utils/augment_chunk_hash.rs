use anyhow::Result;
use futures::future::try_join_all;
use rolldown_common::{AssetMeta, PreliminaryAsset};
use rolldown_plugin::SharedPluginDriver;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn augment_chunk_hash<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: Vec<PreliminaryAsset>,
) -> Result<Vec<PreliminaryAsset>> {
  try_join_all(chunks.into_iter().map(|chunk| async move {
    if let AssetMeta::Ecma(ecma_meta) = &chunk.meta {
      plugin_driver.augment_chunk_hash(ecma_meta).await.map(|augment_chunk_hash| PreliminaryAsset {
        content: chunk.content,
        map: chunk.map,
        meta: chunk.meta,
        augment_chunk_hash,
        file_dir: chunk.file_dir,
        preliminary_filename: chunk.preliminary_filename,
      })
    } else {
      Ok(PreliminaryAsset {
        content: chunk.content,
        map: chunk.map,
        meta: chunk.meta,
        augment_chunk_hash: None,
        file_dir: chunk.file_dir,
        preliminary_filename: chunk.preliminary_filename,
      })
    }
  }))
  .await
}
