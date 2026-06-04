use std::sync::Arc;

use anyhow::Result;
use arcstr::ArcStr;
use futures::future::try_join_all;
use rolldown_common::{
  InsChunkIdx, InstantiationKind, RollupRenderedChunk, SharedNormalizedBundlerOptions,
};
use rolldown_error::BuildDiagnostic;
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps_owned};
use rolldown_utils::indexmap::FxIndexMap;

use crate::type_alias::IndexInstantiatedChunks;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn render_chunks(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexInstantiatedChunks,
  options: &SharedNormalizedBundlerOptions,
) -> Result<Vec<BuildDiagnostic>> {
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
      .collect::<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>(),
  );

  let result = try_join_all(assets.iter_mut().enumerate().map(|(index, asset)| {
    let chunks = Arc::clone(&chunks);
    async move {
      if let InstantiationKind::Ecma(ecma_meta) = &asset.kind {
        let render_chunk_ret = plugin_driver
          .render_chunk(HookRenderChunkArgs {
            code: Arc::new(std::mem::take(&mut asset.content).try_into_inner_string()?),
            chunk: Arc::clone(
              chunks.get(&ecma_meta.rendered_chunk.filename).expect("should have chunk"),
            ),
            options,
            chunks,
          })
          .await?;
        return Ok(Some((index.into(), render_chunk_ret)));
      }

      Ok::<Option<(InsChunkIdx, (String, Vec<SourceMap>, Vec<BuildDiagnostic>))>, anyhow::Error>(
        None,
      )
    }
  }))
  .await?;

  let mut warnings = vec![];
  for (index, (code, sourcemaps, chunk_warnings)) in result.into_iter().flatten() {
    let asset = &mut assets[index];
    asset.content = code.into();
    if !sourcemaps.is_empty() {
      if let Some(asset_map) = asset.map.take() {
        // Move `asset_map` into the collapse so its (chunk-sized) `source_contents`
        // is reused rather than cloned; the plugin-returned maps remap on top.
        let rest: Vec<&SourceMap> = sourcemaps.iter().collect();
        asset.map = Some(collapse_sourcemaps_owned(asset_map, &rest));
      }
    }
    warnings.extend(chunk_warnings);
  }

  Ok(warnings)
}
