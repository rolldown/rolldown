use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use arcstr::ArcStr;
use futures::{StreamExt as _, TryStreamExt as _};
use rolldown_common::{
  ChunkTable, InstantiationKind, ModuleTable, RollupRenderedChunk, SharedNormalizedBundlerOptions,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};
use rolldown_utils::indexmap::FxIndexMap;

use crate::{
  stages::generate_stage::minify_chunks::ChunkMinifier, type_alias::IndexInstantiatedChunks,
};

type RenderChunkFuture<'a> =
  Pin<Box<dyn Future<Output = Result<(usize, Vec<BuildDiagnostic>)>> + Send + 'a>>;

#[tracing::instrument(level = "debug", skip_all)]
/// Calls `renderChunk` for every chunk, then minifies each chunk when `minifier` is given.
///
/// A chunk is minified as soon as its own `renderChunk` hooks return, on the rayon pool, so
/// minification overlaps the hooks of the remaining chunks. JavaScript hooks run one at a time
/// on the main thread, and waiting for all of them before minifying anything left the pool idle
/// for that whole window and then made the largest chunk the tail of a separate minify phase.
/// Each chunk still goes through the same steps in the same order: `renderChunk` hooks, chaining
/// their sourcemaps, then minification.
pub async fn render_chunks(
  plugin_driver: &SharedPluginDriver,
  assets: &mut IndexInstantiatedChunks,
  options: &SharedNormalizedBundlerOptions,
  chunk_table: &ChunkTable,
  module_table: &ModuleTable,
  minifier: Option<&Arc<ChunkMinifier>>,
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

  // Every chunk passes through each plugin's `renderChunk` in turn. Starting all chunks at once
  // queues every chunk's first JavaScript hook ahead of any chunk's second one, so nearly all
  // chunks would finish their hooks together at the very end. Keeping a bounded number of
  // chunks in flight lets them finish one after another while still keeping the main thread
  // busy, and starting with the largest chunks lets their minification, the longest, begin
  // first. Results are put back in chunk order, so warnings keep their order.
  let concurrency = if minifier.is_some() { rayon::current_num_threads() * 4 } else { usize::MAX };
  let mut order = assets.iter_mut().enumerate().collect::<Vec<_>>();
  if minifier.is_some() {
    order.sort_by_key(|(_, asset)| std::cmp::Reverse(asset.content.as_bytes().len()));
  }
  let futures = order
    .into_iter()
    .map(|(index, asset)| -> RenderChunkFuture<'_> {
      let chunks = Arc::clone(&chunks);
      Box::pin(async move {
        let InstantiationKind::Ecma(ecma_meta) = &asset.kind else {
          return Ok::<(usize, Vec<BuildDiagnostic>), anyhow::Error>((index, vec![]));
        };
        let (code, sourcemaps, chunk_warnings) = plugin_driver
          .render_chunk(HookRenderChunkArgs {
            code: Arc::new(std::mem::take(&mut asset.content).try_into_inner_string()?),
            chunk: Arc::clone(
              chunks.get(&ecma_meta.rendered_chunk.filename).expect("should have chunk"),
            ),
            options,
            chunk_table,
            module_table,
            chunks,
          })
          .await?;

        match minifier {
          Some(minifier) if ChunkMinifier::should_minify(asset) => {
            let job = MinifyJob {
              code,
              map: asset.map.take(),
              sourcemaps,
              filename: asset.preliminary_filename.to_string(),
            };
            let (code, map) = run_minify_job(Arc::clone(minifier), job).await?;
            asset.content = code.into();
            asset.map = map;
          }
          _ => {
            asset.content = code.into();
            if let Some(map) = chain_render_chunk_sourcemaps(asset.map.as_ref(), &sourcemaps) {
              asset.map = Some(map);
            }
          }
        }
        Ok((index, chunk_warnings))
      })
    })
    .collect::<Vec<_>>();
  // Boxed above: a stream of these borrowing futures is otherwise not provably `Send`.
  let mut result =
    futures::stream::iter(futures).buffer_unordered(concurrency).try_collect::<Vec<_>>().await?;

  result.sort_unstable_by_key(|(index, _)| *index);
  Ok(result.into_iter().flat_map(|(_, warnings)| warnings).collect())
}

/// The chunk's map with the maps returned by its `renderChunk` hooks chained on, or `None` when
/// there is nothing to chain.
fn chain_render_chunk_sourcemaps(
  asset_map: Option<&SourceMap>,
  sourcemaps: &[SourceMap],
) -> Option<SourceMap> {
  let asset_map = asset_map?;
  if sourcemaps.is_empty() {
    return None;
  }
  let mut sourcemap_chain = Vec::with_capacity(sourcemaps.len() + 1);
  sourcemap_chain.push(asset_map);
  sourcemap_chain.extend(sourcemaps.iter());
  Some(collapse_sourcemaps(&sourcemap_chain))
}

/// One chunk after its `renderChunk` hooks: its code, its map, and the hooks' maps.
struct MinifyJob {
  code: String,
  map: Option<SourceMap>,
  sourcemaps: Vec<SourceMap>,
  filename: String,
}

impl MinifyJob {
  fn run(self, minifier: &ChunkMinifier) -> BuildResult<(String, Option<SourceMap>)> {
    let map = match chain_render_chunk_sourcemaps(self.map.as_ref(), &self.sourcemaps) {
      Some(chained) => Some(chained),
      None => self.map,
    };
    let (code, minified_map) = minifier.minify(&self.code, map.as_ref(), &self.filename)?;
    // The chunk keeps its map when minification makes none, as before.
    Ok((code, minified_map.or(map)))
  }
}

#[cfg(not(target_family = "wasm"))]
async fn run_minify_job(
  minifier: Arc<ChunkMinifier>,
  job: MinifyJob,
) -> Result<(String, Option<SourceMap>)> {
  let (sender, receiver) = futures::channel::oneshot::channel();
  rayon::spawn(move || {
    let result = job.run(&minifier);
    // Release the minifier before reporting back, so it is no longer shared once every job's
    // result has arrived and `render_chunk_to_assets` can take its property mangle cache.
    drop(minifier);
    // The receiver only goes away when the build already failed.
    let _ = sender.send(result);
  });
  Ok(receiver.await.expect("minify job dropped its result")?)
}

#[cfg(target_family = "wasm")]
async fn run_minify_job(
  minifier: Arc<ChunkMinifier>,
  job: MinifyJob,
) -> Result<(String, Option<SourceMap>)> {
  Ok(job.run(&minifier)?)
}
