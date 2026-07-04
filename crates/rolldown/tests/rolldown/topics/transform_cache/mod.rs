use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rolldown::{
  Bundler, BundlerOptions, ExperimentalOptions, InputItem, TransformCacheOption,
  TransformCacheOptions,
};
use rolldown_common::Output;
use rolldown_plugin::{HookTransformOutputMap, HookUsage, Plugin};

/// Counts its `transform` invocations on the entry module and rewrites the
/// `__MARKER__` placeholder, so cache hits are observable both through the
/// counter (hooks skipped) and the emitted chunk (cached code used).
#[derive(Debug)]
struct CountingTransformPlugin {
  calls: Arc<AtomicUsize>,
}

impl Plugin for CountingTransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    "counting-transform".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !args.id.ends_with("entry.js") {
      return Ok(None);
    }
    self.calls.fetch_add(1, Ordering::SeqCst);
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(args.code.replace("__MARKER__", "transformed")),
      map: HookTransformOutputMap::Null,
      ..Default::default()
    }))
  }
}

fn fixture_cwd() -> PathBuf {
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/transform_cache").into()
}

/// Builds the fixture with the persistent transform cache enabled and returns
/// how often the transform hook ran plus the entry chunk's code.
async fn build(cache_dir: &std::path::Path, key: Option<&str>) -> (usize, String) {
  let calls = Arc::new(AtomicUsize::new(0));
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      cwd: Some(fixture_cwd()),
      experimental: Some(ExperimentalOptions {
        transform_cache: Some(TransformCacheOption::Options(TransformCacheOptions {
          dir: Some(cache_dir.to_string_lossy().into_owned()),
          key: key.map(str::to_string),
        })),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Arc::new(CountingTransformPlugin { calls: Arc::clone(&calls) })],
  )
  .expect("failed to create bundler");

  let output = bundler.generate().await.expect("build should succeed");
  let code = output
    .assets
    .iter()
    .find_map(|asset| match asset {
      Output::Chunk(chunk) => Some(chunk.code.clone()),
      Output::Asset(_) => None,
    })
    .expect("build should emit a chunk");
  (calls.load(Ordering::SeqCst), code)
}

fn fresh_cache_dir(name: &str) -> PathBuf {
  let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("transform_cache").join(name);
  let _ = std::fs::remove_dir_all(&dir);
  dir
}

#[tokio::test(flavor = "multi_thread")]
async fn second_build_skips_transform_hooks_and_reuses_cached_output() {
  let cache_dir = fresh_cache_dir("hit");

  let (first_calls, first_code) = Box::pin(build(&cache_dir, None)).await;
  assert_eq!(first_calls, 1, "cold build must run the transform hook");
  assert!(first_code.contains("transformed"), "transform result must reach the chunk");

  let (second_calls, second_code) = Box::pin(build(&cache_dir, None)).await;
  assert_eq!(second_calls, 0, "warm build must skip the transform hook entirely");
  assert_eq!(second_code, first_code, "cached build must produce identical output");
}

#[tokio::test(flavor = "multi_thread")]
async fn changing_cache_key_invalidates_entries() {
  let cache_dir = fresh_cache_dir("key");

  let (first_calls, _) = Box::pin(build(&cache_dir, Some("config-hash-a"))).await;
  assert_eq!(first_calls, 1);

  let (same_key_calls, _) = Box::pin(build(&cache_dir, Some("config-hash-a"))).await;
  assert_eq!(same_key_calls, 0, "same key must hit the cache");

  let (new_key_calls, _) = Box::pin(build(&cache_dir, Some("config-hash-b"))).await;
  assert_eq!(new_key_calls, 1, "a new key must invalidate every entry");
}
