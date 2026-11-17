use std::{cmp::Reverse, future::Future, sync::Arc};

use anyhow::Result;
use arcstr::ArcStr;
use futures::future::try_join_all;
use rolldown_common::{
  ChunkTable, InsChunkIdx, InstantiatedChunk, ModuleTable, RollupRenderedChunk,
  SharedNormalizedBundlerOptions,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::{HookRenderChunkArgs, SharedPluginDriver};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};
use rolldown_utils::indexmap::FxIndexMap;

use crate::{
  stages::generate_stage::minify_chunks::ChunkMinifier, type_alias::IndexInstantiatedChunks,
};

#[tracing::instrument(level = "debug", skip_all)]
/// Calls `renderChunk` for every chunk, then minifies each chunk when `minifier` is given.
///
/// A chunk's minification starts on the rayon pool as soon as its own `renderChunk` hooks
/// return, so it overlaps the hooks of the other chunks, which run one at a time on the main
/// thread when they are JavaScript hooks. The results are taken only after every chunk's hooks
/// have returned, so an error from a hook is reported before an error from minification.
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
      .map(|asset| {
        let rendered_chunk = &asset.ecma_meta.rendered_chunk;
        (rendered_chunk.filename.clone(), Arc::clone(rendered_chunk))
      })
      .collect::<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>(),
  );

  // Every chunk starts its `renderChunk` hooks at once, so a hook may wait for other chunks. With
  // more than 16 chunks, a chunk holding more than a sixteenth of the code to minify starts first:
  // it reaches each hook first, so its minification, the longest, starts first. 16 is fixed rather
  // than the thread count, so the order of calls is the same on every machine.
  const SHARES: usize = 16;
  let minified_len = |asset: &InstantiatedChunk| {
    if ChunkMinifier::should_minify(asset) { asset.content.as_bytes().len() } else { 0 }
  };
  let mut order = assets.iter_mut().enumerate().collect::<Vec<_>>();
  if minifier.is_some() && order.len() > SHARES {
    let share = order.iter().map(|(_, asset)| minified_len(asset)).sum::<usize>() / SHARES;
    // A stable sort, so the other chunks keep chunk order.
    order.sort_by_key(|(_, asset)| {
      let len = minified_len(asset);
      Reverse(if len > share { len } else { 0 })
    });
  }
  let render_chunk_order = &plugin_driver.render_chunk_order(order.len());
  let order = order.into_iter().enumerate();
  let mut rendered = try_join_all(order.map(|(position, (index, asset))| {
    let chunks = Arc::clone(&chunks);
    async move {
      let (code, sourcemaps, chunk_warnings) = plugin_driver
        .render_chunk(
          render_chunk_order,
          position,
          HookRenderChunkArgs {
            code: Arc::new(std::mem::take(&mut asset.content).try_into_inner_string()?),
            chunk: Arc::clone(
              chunks.get(&asset.ecma_meta.rendered_chunk.filename).expect("should have chunk"),
            ),
            options,
            chunk_table,
            module_table,
            chunks,
          },
        )
        .await?;

      let minified = match minifier {
        Some(minifier) if ChunkMinifier::should_minify(asset) => {
          let job = MinifyJob {
            code,
            map: asset.map.take(),
            sourcemaps,
            filename: asset.preliminary_filename.to_string(),
          };
          let minifier = Arc::clone(minifier);
          Some(spawn_on_rayon(move || job.run(&minifier)))
        }
        _ => {
          asset.content = code.into();
          if let Some(map) = chain_render_chunk_sourcemaps(asset.map.as_ref(), &sourcemaps) {
            asset.map = Some(map);
          }
          None
        }
      };
      Ok::<_, anyhow::Error>((index, chunk_warnings, minified))
    }
  }))
  .await?;

  // Taken in chunk order, so warnings keep their order.
  rendered.sort_unstable_by_key(|(index, ..)| *index);
  let mut warnings = vec![];
  for (index, chunk_warnings, minified) in rendered {
    if let Some(minified) = minified {
      let (code, map) = minified.await?;
      let asset = &mut assets[InsChunkIdx::from(index)];
      asset.content = code.into();
      asset.map = map;
    }
    warnings.extend(chunk_warnings);
  }
  Ok(warnings)
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
    // The chunk keeps its map when minification makes none.
    Ok((code, minified_map.or(map)))
  }
}

/// Starts `f` on the rayon pool; the returned future yields its result. Everything `f` captured
/// is dropped before the result is sent. A panic in `f` is resumed where the result is awaited,
/// where it fails the build as any other panic does: the pool has no panic handler, so a panic in
/// a spawned job would abort the process.
#[cfg(not(target_family = "wasm"))]
fn spawn_on_rayon<T: Send + 'static>(
  f: impl FnOnce() -> T + Send + 'static,
) -> impl Future<Output = T> {
  let (sender, receiver) = futures::channel::oneshot::channel();
  rayon::spawn(move || {
    // The receiver only goes away when the build already failed.
    let _ = sender.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)));
  });
  async move {
    match receiver.await.expect("the job always sends its result") {
      Ok(value) => value,
      Err(panic) => std::panic::resume_unwind(panic),
    }
  }
}

#[cfg(target_family = "wasm")]
fn spawn_on_rayon<T>(f: impl FnOnce() -> T) -> impl Future<Output = T> {
  std::future::ready(f())
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
  use super::spawn_on_rayon;

  #[tokio::test]
  async fn a_panic_in_a_rayon_job_reaches_the_caller() {
    let result = tokio::spawn(spawn_on_rayon(|| -> () { panic!("minifier bug") })).await;
    let panic = result.expect_err("the panic reaches the awaiting task").into_panic();
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"minifier bug"));
  }
}
