use anyhow::Result;
use futures::future::try_join_all;
use rolldown_common::AssetMeta;
use rolldown_plugin::SharedPluginDriver;

use crate::type_alias::IndexPreliminaryAssets;
//
#[tracing::instrument(level = "debug", skip_all)]
pub async fn augment_chunk_hash<'a>(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexPreliminaryAssets,
) -> Result<()> {
  try_join_all(assets.iter_mut().map(|asset| async move {
    if let AssetMeta::Ecma(ecma_meta) = &asset.meta {
      let augment_chunk_hash = plugin_driver.augment_chunk_hash(&ecma_meta.rendered_chunk).await?;
      if let Some(augment_chunk_hash) = augment_chunk_hash {
        asset.augment_chunk_hash = Some(augment_chunk_hash);
      }
    }

    Ok::<(), anyhow::Error>(())
  }))
  .await?;

  Ok(())
}
