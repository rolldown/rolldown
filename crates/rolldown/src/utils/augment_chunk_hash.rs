use anyhow::{Ok, Result};
use futures::future::try_join_all;
use rolldown_common::{AssetMeta, PreliminaryAsset};
use rolldown_plugin::SharedPluginDriver;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn augment_chunk_hash<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: Vec<PreliminaryAsset>,
) -> Result<Vec<PreliminaryAsset>> {
  try_join_all(chunks.into_iter().map(|mut asset| async move {
    let augment_chunk_hash = if let AssetMeta::Ecma(ecma_meta) = &asset.meta {
      plugin_driver.augment_chunk_hash(&ecma_meta.rendered_chunk).await
    } else {
      Ok(None)
    }?;

    if let Some(augment_chunk_hash) = augment_chunk_hash {
      asset.augment_chunk_hash = Some(augment_chunk_hash);
    }

    Ok(asset)
  }))
  .await
}
