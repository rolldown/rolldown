use std::sync::Arc;

use futures::future::try_join_all;
use rolldown_common::InstantiationKind;
use rolldown_error::{BuildDiagnostic, SingleBuildResult};
use rolldown_plugin::SharedPluginDriver;

use crate::type_alias::IndexInstantiatedChunks;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn augment_chunk_hash(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexInstantiatedChunks,
) -> SingleBuildResult<()> {
  try_join_all(assets.iter_mut().map(|asset| async move {
    if let InstantiationKind::Ecma(ecma_meta) = &asset.kind {
      let augment_chunk_hash =
        plugin_driver.augment_chunk_hash(Arc::clone(&ecma_meta.rendered_chunk)).await?;
      if let Some(augment_chunk_hash) = augment_chunk_hash {
        asset.augment_chunk_hash = Some(augment_chunk_hash);
      }
    }

    Ok::<(), BuildDiagnostic>(())
  }))
  .await?;

  Ok(())
}
