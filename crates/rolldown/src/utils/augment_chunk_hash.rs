use anyhow::Result;
use futures::future::try_join_all;
use rolldown_common::InstantiationKind;
use rolldown_plugin::SharedPluginDriver;

use crate::type_alias::IndexInstantiatedChunks;
//
#[tracing::instrument(level = "debug", skip_all)]
pub async fn augment_chunk_hash<'a>(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexInstantiatedChunks,
) -> Result<()> {
  try_join_all(assets.iter_mut().map(|asset| async move {
    if let InstantiationKind::Ecma(ecma_meta) = &asset.meta {
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
