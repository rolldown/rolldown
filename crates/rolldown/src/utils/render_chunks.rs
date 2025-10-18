use std::sync::Arc;

use anyhow::Result;
use arcstr::ArcStr;
use futures::future::{Either, FutureExt, try_join_all};
use rolldown_common::{
  InsChunkIdx, InstantiationKind, RollupRenderedChunk, SharedNormalizedBundlerOptions,
};
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};
use rustc_hash::FxHashMap;

use crate::type_alias::IndexInstantiatedChunks;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn render_chunks(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexInstantiatedChunks,
  options: &SharedNormalizedBundlerOptions,
) -> Result<()> {
  let chunks = Arc::new(
    assets
      .iter()
      .filter_map(|asset| {
        if let InstantiationKind::Ecma(ecma_meta) = &asset.kind {
          Some((ecma_meta.rendered_chunk.filename.clone(), Arc::clone(&ecma_meta.rendered_chunk)))
        } else {
          None
        }
      })
      .collect::<FxHashMap<ArcStr, Arc<RollupRenderedChunk>>>(),
  );

  let result = try_join_all(assets.iter_mut().enumerate().map(|(index, asset)| {
    let chunks = Arc::clone(&chunks);
    if let InstantiationKind::Ecma(ecma_meta) = &asset.kind {
      let plugin_driver = Arc::clone(plugin_driver);
      let options = Arc::clone(options);
      let asset_content = std::mem::take(&mut asset.content);
      let Some(chunk) = chunks.get(&ecma_meta.rendered_chunk.filename) else {
        return Either::Right(futures::future::ready(Err(anyhow::anyhow!("should have chunk"))));
      };

      let chunk = Arc::clone(chunk);
      Either::Left(
        tokio::spawn(async move {
          let render_chunk_ret = plugin_driver
            .render_chunk(HookRenderChunkArgs {
              code: asset_content.try_into_inner_string()?,
              chunk,
              options: &options,
              chunks,
            })
            .await?;
          Ok::<_, anyhow::Error>(render_chunk_ret)
        })
        .map(move |ret| {
          ret
            .map_err(anyhow::Error::from)
            .and_then(|ret| ret.map(|render_chunk_ret| Some((index.into(), render_chunk_ret))))
        }),
      )
    } else {
      Either::Right(futures::future::ready(Ok::<
        Option<(InsChunkIdx, (String, Vec<SourceMap>))>,
        anyhow::Error,
      >(None)))
    }
  }))
  .await?;

  for (index, (code, sourcemaps)) in result.into_iter().flatten() {
    let asset = &mut assets[index];
    asset.content = code.into();
    if !sourcemaps.is_empty() {
      if let Some(asset_map) = &asset.map {
        let mut sourcemap_chain = Vec::with_capacity(sourcemaps.len() + 1);
        sourcemap_chain.push(asset_map);
        sourcemap_chain.extend(sourcemaps.iter());
        asset.map = Some(collapse_sourcemaps(&sourcemap_chain));
      }
    }
  }

  Ok(())
}
