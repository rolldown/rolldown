use anyhow::Result;
use futures::future::try_join_all;
use rolldown_common::{AssetMeta, PreliminaryAsset};
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::collapse_sourcemaps;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn render_chunks<'a>(
  plugin_driver: &SharedPluginDriver,
  chunks: Vec<PreliminaryAsset>,
) -> Result<Vec<PreliminaryAsset>> {
  try_join_all(chunks.into_iter().map(|mut asset| async move {
    // TODO(hyf0): To be refactor:
    // - content should use ArcStr
    // - plugin_driver.render_chunk should return Option<...> to be able to see if there is a return value by the plugin
    let render_chunk_ret = if let AssetMeta::Ecma(ecma_meta) = &asset.meta {
      plugin_driver
        .render_chunk(HookRenderChunkArgs { code: asset.content.clone(), chunk: ecma_meta })
        .await
    } else {
      Ok((asset.content.clone(), vec![]))
    }?;

    asset.content = render_chunk_ret.0;
    if let Some(asset_map) = &asset.map {
      if !render_chunk_ret.1.is_empty() {
        let mut sourcemap_chain = Vec::with_capacity(render_chunk_ret.1.len() + 1);
        sourcemap_chain.push(asset_map);
        sourcemap_chain.extend(render_chunk_ret.1.iter());
        let new_source_map = collapse_sourcemaps(sourcemap_chain);
        asset.map = new_source_map;
      }
    }

    Ok(asset)
  }))
  .await
}
