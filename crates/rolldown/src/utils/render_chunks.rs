use std::sync::Arc;

use anyhow::Result;
use arcstr::ArcStr;
use futures::future::try_join_all;
use rolldown_common::{
  InsChunkIdx, InstantiationKind, RollupRenderedChunk, SharedNormalizedBundlerOptions,
};
use rolldown_error::BuildDiagnostic;
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};
use rustc_hash::FxHashMap;

use crate::type_alias::IndexInstantiatedChunks;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn render_chunks(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexInstantiatedChunks,
  options: &SharedNormalizedBundlerOptions,
  warnings: &mut Vec<BuildDiagnostic>,
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
    async move {
      if let InstantiationKind::Ecma(ecma_meta) = &asset.kind {
        let render_chunk_ret = plugin_driver
          .render_chunk(HookRenderChunkArgs {
            code: std::mem::take(&mut asset.content).try_into_inner_string()?,
            chunk: Arc::clone(
              chunks.get(&ecma_meta.rendered_chunk.filename).expect("should have chunk"),
            ),
            options,
            chunks,
          })
          .await?;

        return Ok(Some((index.into(), render_chunk_ret)));
      }

      Ok::<Option<(InsChunkIdx, (String, Vec<SourceMap>, Vec<String>))>, anyhow::Error>(None)
    }
  }))
  .await?;

  for (index, (code, sourcemaps, plugin_names)) in result.into_iter().flatten() {
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

    if options.sourcemap.is_some() {
      let sourcemap_type =
        options.sourcemap.as_ref().map(rolldown_common::SourceMapType::as_str).unwrap();

      if sourcemap_type == "file" || sourcemap_type == "inline" {
        if !plugin_names.is_empty() {
          plugin_names.iter().for_each(|plugin_name| {
            warnings.push(
              BuildDiagnostic::sourcemap_broken(plugin_name.as_str(), "load", sourcemap_type)
                .with_severity_warning(),
            );
          });
        }
      }
    }
  }

  Ok(())
}
