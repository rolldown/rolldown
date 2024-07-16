use anyhow::Result;
use futures::future::try_join_all;
use rolldown_plugin::{HookBannerArgs, SharedPluginDriver};

use crate::type_alias::IndexPreliminaryAssets;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn banner<'a>(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexPreliminaryAssets,
) -> Result<()> {
  try_join_all(assets.iter_mut().map(|asset| async move {
    plugin_driver.banner(HookBannerArgs { code: asset.content.clone() }).await.map(|code| {
      println!("banner code: {:#?}", code);
    });

    Ok::<(), anyhow::Error>(())
  }))
  .await?;

  Ok(())
}
