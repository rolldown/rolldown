use anyhow::Result;
use futures::future::try_join_all;
use rolldown_common::AssetMeta;
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::collapse_sourcemaps;

use crate::type_alias::IndexPreliminaryAssets;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn render_chunks<'a>(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexPreliminaryAssets,
) -> Result<()> {
  try_join_all(assets.iter_mut().map(|asset| async move {
    // TODO(hyf0): To be refactor:
    // - content should use ArcStr
    // - plugin_driver.render_chunk should return Option<...> to be able to see if there is a return value by the plugin
    if let AssetMeta::Ecma(ecma_meta) = &asset.meta {
      let render_chunk_ret = plugin_driver
        .render_chunk(HookRenderChunkArgs {
          code: asset.content.clone(),
          chunk: &ecma_meta.rendered_chunk,
        })
        .await?;

      asset.content = render_chunk_ret.0;
      if let Some(asset_map) = &asset.map {
        if !render_chunk_ret.1.is_empty() {
          let mut sourcemap_chain = Vec::with_capacity(render_chunk_ret.1.len() + 1);
          sourcemap_chain.push(asset_map);
          sourcemap_chain.extend(render_chunk_ret.1.iter());
          asset.map = Some(collapse_sourcemaps(sourcemap_chain));
        }
      }
    }

    Ok::<(), anyhow::Error>(())
  }))
  .await?;

  Ok(())
}
