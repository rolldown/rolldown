use std::{borrow::Cow, sync::Arc};

use arcstr::ArcStr;
use rolldown::{AssetFilenamesOutputOption, Bundler, BundlerOptions, InputItem};
use rolldown_common::{EmittedAsset, Output, OutputAsset};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};

/// Identical content shared by every deduplicated asset; only the name varies.
const SHARED_SOURCE: &str = "shared";

/// One `this.emitFile({ type: 'asset' })` call.
#[derive(Debug, Default)]
struct Emit {
  name: Option<String>,
  original_file_name: Option<String>,
  file_name: Option<String>,
  source: String,
}

impl Emit {
  /// A deduplicated asset: shared content, no explicit file name.
  fn dedup(name: &str) -> Self {
    Self { name: Some(name.into()), source: SHARED_SOURCE.into(), ..Default::default() }
  }

  /// An asset with an explicit file name, which is never deduplicated.
  fn explicit(file_name: &str) -> Self {
    Self { file_name: Some(file_name.into()), source: SHARED_SOURCE.into(), ..Default::default() }
  }

  fn with_original(mut self, original_file_name: &str) -> Self {
    self.original_file_name = Some(original_file_name.into());
    self
  }

  fn to_emitted(&self) -> EmittedAsset {
    EmittedAsset {
      name: self.name.clone(),
      original_file_name: self.original_file_name.clone(),
      file_name: self.file_name.clone().map(ArcStr::from),
      source: self.source.clone().into(),
    }
  }
}

/// Emits the configured assets from `build_start`. When `concurrent`, the emits
/// run on parallel threads, exercising the deduplication race the fix closes.
#[derive(Debug)]
struct EmitPlugin {
  emits: Vec<Emit>,
  concurrent: bool,
}

impl Plugin for EmitPlugin {
  fn name(&self) -> Cow<'static, str> {
    "emit".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.concurrent {
      std::thread::scope(|scope| {
        for emit in &self.emits {
          scope.spawn(move || {
            ctx.emit_file(emit.to_emitted(), None, None).expect("emit_file failed");
          });
        }
      });
    } else {
      for emit in &self.emits {
        ctx.emit_file(emit.to_emitted(), None, None)?;
      }
    }
    Ok(())
  }
}

/// Bundles a dummy entry with `plugin` and returns the emitted output assets.
async fn bundle(plugin: EmitPlugin) -> Vec<Arc<OutputAsset>> {
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
    vec![Arc::new(plugin)],
  )
  .expect("failed to create bundler");

  bundler
    .generate()
    .await
    .expect("build should succeed")
    .assets
    .into_iter()
    .filter_map(|output| match output {
      Output::Asset(asset) => Some(asset),
      Output::Chunk(_) => None,
    })
    .collect()
}

async fn emit_assets(emits: Vec<Emit>) -> Vec<Arc<OutputAsset>> {
  bundle(EmitPlugin { emits, concurrent: false }).await
}

async fn emit_assets_concurrently(emits: Vec<Emit>) -> Vec<Arc<OutputAsset>> {
  bundle(EmitPlugin { emits, concurrent: true }).await
}

/// The surviving file name must be deterministic across emission orders and is
/// the shortest name (ties broken lexicographically), matching Rollup: `z.txt`
/// wins over the longer, lexicographically-first `aaaa.txt`.
#[tokio::test(flavor = "multi_thread")]
async fn dedup_survivor_is_shortest_name_regardless_of_emission_order() {
  let forward = emit_assets(vec![Emit::dedup("aaaa.txt"), Emit::dedup("z.txt")]).await;
  let reversed = emit_assets(vec![Emit::dedup("z.txt"), Emit::dedup("aaaa.txt")]).await;

  assert_eq!(forward.len(), 1, "identical content deduplicates into one asset");
  assert_eq!(forward[0].filename, reversed[0].filename, "survivor must be order-independent");
  assert!(
    forward[0].filename.starts_with("assets/z-"),
    "shortest name must win, got: {}",
    forward[0].filename
  );
}

/// The shortest name wins even when it is not the lexicographically-first one.
#[tokio::test(flavor = "multi_thread")]
async fn dedup_survivor_prefers_shortest_over_lexicographic() {
  let a =
    emit_assets(vec![Emit::dedup("zz.txt"), Emit::dedup("m.txt"), Emit::dedup("aaa.txt")]).await;
  let b =
    emit_assets(vec![Emit::dedup("aaa.txt"), Emit::dedup("zz.txt"), Emit::dedup("m.txt")]).await;

  assert_eq!(a[0].filename, b[0].filename, "survivor must be order-independent");
  assert!(
    a[0].filename.starts_with("assets/m-"),
    "shortest name `m.txt` must beat lexicographically-first `aaa.txt`, got: {}",
    a[0].filename
  );
}

/// Every duplicate's `name` and `original_file_name` is collected onto the single
/// surviving asset, each sorted deterministically.
#[tokio::test(flavor = "multi_thread")]
async fn dedup_collects_names_and_original_file_names() {
  let assets = emit_assets(vec![
    Emit::dedup("b.txt").with_original("src/b.txt"),
    Emit::dedup("a.txt").with_original("src/a.txt"),
    Emit::dedup("cc.txt").with_original("src/cc.txt"),
  ])
  .await;

  assert_eq!(assets.len(), 1, "identical content deduplicates into one asset");
  let names: Vec<&str> = assets[0].names.iter().map(String::as_str).collect();
  let original: Vec<&str> = assets[0].original_file_names.iter().map(String::as_str).collect();
  assert_eq!(names, ["a.txt", "b.txt", "cc.txt"], "names sorted shortest-then-lexicographic");
  assert_eq!(original, ["src/a.txt", "src/b.txt", "src/cc.txt"], "original file names sorted");
  assert!(assets[0].filename.starts_with("assets/a-"), "filename derived from the winning name");
}

/// Assets with an explicit `file_name` are never deduplicated and keep their name
/// verbatim, even with identical content.
#[tokio::test(flavor = "multi_thread")]
async fn explicit_file_name_skips_deduplication() {
  let mut assets = emit_assets(vec![Emit::explicit("foo.txt"), Emit::explicit("bar.txt")]).await;

  assets.sort_by(|a, b| a.filename.cmp(&b.filename));
  let filenames: Vec<&str> = assets.iter().map(|asset| asset.filename.as_str()).collect();
  assert_eq!(
    filenames,
    ["bar.txt", "foo.txt"],
    "explicit file names bypass dedup and stay verbatim"
  );
}

/// Concurrent deduplication must not drop any duplicate's metadata: the emitter
/// inserts the asset while holding the source-hash shard lock, so a concurrent
/// duplicate always finds it. Every emitted name must survive.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_dedup_keeps_every_name() {
  const COUNT: usize = 32;
  let emits = (0..COUNT).map(|i| Emit::dedup(&format!("name{i:02}.txt"))).collect();
  let assets = emit_assets_concurrently(emits).await;

  assert_eq!(assets.len(), 1, "identical content deduplicates into one asset");
  assert_eq!(assets[0].names.len(), COUNT, "no concurrently-emitted name may be lost");
}
