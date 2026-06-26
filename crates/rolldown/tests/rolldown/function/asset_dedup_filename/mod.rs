use std::{borrow::Cow, sync::Arc};

use rolldown::{AssetFilenamesOutputOption, Bundler, BundlerOptions, InputItem};
use rolldown_common::{EmittedAsset, Output};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};

/// All emitted assets share this exact `source`, so the file emitter
/// deduplicates them into a single output asset whose surviving file name must
/// depend only on the names, not the (parallel, non-deterministic) emission order.
const SHARED_SOURCE: &str = "shared-asset-bytes-used-for-deduplication";

/// Emits one asset per `name` (in order), all with [`SHARED_SOURCE`] and no
/// explicit `file_name`.
#[derive(Debug)]
struct EmitDuplicatesPlugin {
  names: Vec<&'static str>,
}

impl Plugin for EmitDuplicatesPlugin {
  fn name(&self) -> Cow<'static, str> {
    "emit-duplicates".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    for name in &self.names {
      ctx.emit_file(
        EmittedAsset {
          file_name: None,
          original_file_name: None,
          name: Some((*name).to_string()),
          source: SHARED_SOURCE.to_string().into(),
        },
        None,
        None,
      )?;
    }
    Ok(())
  }
}

/// Bundles a dummy entry while emitting identical-content assets named
/// `emit_order`, then returns the single deduplicated asset's file name.
async fn dedup_survivor(emit_order: Vec<&'static str>) -> String {
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      cwd: Some(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/asset_dedup_filename").into(),
      ),
      asset_filenames: Some(AssetFilenamesOutputOption::String(
        "assets/[name]-[hash][extname]".into(),
      )),
      ..Default::default()
    },
    vec![Arc::new(EmitDuplicatesPlugin { names: emit_order })],
  )
  .expect("failed to create bundler");

  let output = bundler.generate().await.expect("build should succeed");
  let assets: Vec<_> = output
    .assets
    .iter()
    .filter_map(|o| match o {
      Output::Asset(asset) => Some(asset.filename.to_string()),
      Output::Chunk(_) => None,
    })
    .collect();
  assert_eq!(assets.len(), 1, "identical-content assets must deduplicate into one asset");
  assets.into_iter().next().unwrap()
}

/// The surviving file name must be the same regardless of which duplicate is
/// emitted first, and it must be the *shortest* name (then lexicographic) to
/// match Rollup. Here `z.txt` must win over the longer, lexicographically-first
/// `aaaa.txt` even though `aaaa.txt` is emitted first.
#[tokio::test(flavor = "multi_thread")]
async fn dedup_survivor_is_shortest_name_regardless_of_emission_order() {
  let forward = dedup_survivor(vec!["aaaa.txt", "z.txt"]).await;
  let reversed = dedup_survivor(vec!["z.txt", "aaaa.txt"]).await;

  assert_eq!(
    forward, reversed,
    "deduplicated file name must be deterministic across emission orders"
  );
  assert!(
    forward.starts_with("assets/z-") && forward.ends_with(".txt"),
    "shortest name must win (Rollup-compatible), got: {forward}"
  );
}

/// With three names of different lengths, the shortest (`m.txt`) wins over both
/// the longer and the lexicographically-smaller candidates, independent of order.
#[tokio::test(flavor = "multi_thread")]
async fn dedup_survivor_prefers_shortest_over_lexicographic() {
  let a = dedup_survivor(vec!["zz.txt", "m.txt", "aaa.txt"]).await;
  let b = dedup_survivor(vec!["aaa.txt", "zz.txt", "m.txt"]).await;

  assert_eq!(a, b, "deduplicated file name must be deterministic across emission orders");
  assert!(
    a.starts_with("assets/m-"),
    "shortest name `m.txt` must beat lexicographically-first `aaa.txt`, got: {a}"
  );
}
