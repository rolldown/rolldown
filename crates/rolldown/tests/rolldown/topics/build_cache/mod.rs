use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rolldown::{
  BuildCacheOption, BuildCacheOptions, Bundler, BundlerOptions, ExperimentalOptions, InputItem,
};
use rolldown_common::Output;
use rolldown_plugin::{HookTransformOutputMap, HookUsage, Plugin};

/// Counts its `resolveId` (importer-driven only), `load` and `transform`
/// invocations on fixture modules and rewrites the `__MARKER__` placeholder,
/// so cache hits are observable both through the counters (the whole pipeline
/// skipped) and the emitted chunk (cached code used).
#[derive(Debug, Default)]
struct CountingPlugin {
  resolve_calls: Arc<AtomicUsize>,
  load_calls: Arc<AtomicUsize>,
  transform_calls: Arc<AtomicUsize>,
}

impl Plugin for CountingPlugin {
  fn name(&self) -> Cow<'static, str> {
    "counting".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::Transform
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    // Entry points resolve on every build; only dependency resolution is
    // covered by the cache.
    if args.importer.is_some() {
      self.resolve_calls.fetch_add(1, Ordering::SeqCst);
    }
    Ok(None)
  }

  async fn load(
    &self,
    _ctx: rolldown_plugin::SharedLoadPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if is_fixture_module(args.id) {
      self.load_calls.fetch_add(1, Ordering::SeqCst);
    }
    Ok(None)
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !is_fixture_module(args.id) {
      return Ok(None);
    }
    self.transform_calls.fetch_add(1, Ordering::SeqCst);
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(args.code.replace("__MARKER__", "transformed")),
      map: HookTransformOutputMap::Null,
      ..Default::default()
    }))
  }
}

/// The virtual runtime module (`\0rolldown/runtime.js`) runs its hooks on
/// every build by design; only real fixture files are counted.
fn is_fixture_module(id: &str) -> bool {
  id.ends_with(".js") && !id.starts_with('\0')
}

struct BuildResult {
  resolve_calls: usize,
  load_calls: usize,
  transform_calls: usize,
  code: String,
}

/// Builds `entry.js` in `cwd` with the persistent build cache enabled and
/// returns per-hook invocation counts plus the entry chunk's code.
async fn build(cwd: &Path, cache_dir: &Path, key: Option<&str>) -> anyhow::Result<BuildResult> {
  let plugin = Arc::new(CountingPlugin::default());
  let (resolve_calls, load_calls, transform_calls) = (
    Arc::clone(&plugin.resolve_calls),
    Arc::clone(&plugin.load_calls),
    Arc::clone(&plugin.transform_calls),
  );
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      cwd: Some(cwd.to_path_buf()),
      experimental: Some(ExperimentalOptions {
        build_cache: Some(BuildCacheOption::Options(BuildCacheOptions {
          dir: Some(cache_dir.to_string_lossy().into_owned()),
          key: key.map(str::to_string),
        })),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![plugin],
  )?;

  let output = bundler.generate().await?;
  let code = output
    .assets
    .iter()
    .find_map(|asset| match asset {
      Output::Chunk(chunk) => Some(chunk.code.clone()),
      Output::Asset(_) => None,
    })
    .expect("build should emit a chunk");
  Ok(BuildResult {
    resolve_calls: resolve_calls.load(Ordering::SeqCst),
    load_calls: load_calls.load(Ordering::SeqCst),
    transform_calls: transform_calls.load(Ordering::SeqCst),
    code,
  })
}

/// Creates a fresh two-module fixture (`entry.js` importing `dep.js`) plus an
/// empty cache dir under the target tmpdir.
fn fresh_fixture(name: &str) -> (PathBuf, PathBuf) {
  let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("build_cache").join(name);
  let _ = std::fs::remove_dir_all(&root);
  let cwd = root.join("src");
  std::fs::create_dir_all(&cwd).unwrap();
  std::fs::write(
    cwd.join("entry.js"),
    "import { dep } from './dep.js';\nconsole.log('__MARKER__', dep);\n",
  )
  .unwrap();
  std::fs::write(cwd.join("dep.js"), "export const dep = '__MARKER__ dep';\n").unwrap();
  (cwd, root.join("cache"))
}

#[tokio::test(flavor = "multi_thread")]
async fn second_build_skips_resolve_load_and_transform_and_reuses_cached_output() {
  let (cwd, cache_dir) = fresh_fixture("hit");

  let first = Box::pin(build(&cwd, &cache_dir, None)).await.unwrap();
  assert_eq!(first.resolve_calls, 1, "cold build must resolve the dependency");
  assert_eq!(first.load_calls, 2, "cold build must load both modules");
  assert_eq!(first.transform_calls, 2, "cold build must transform both modules");
  assert!(first.code.contains("transformed"), "transform result must reach the chunk");

  let second = Box::pin(build(&cwd, &cache_dir, None)).await.unwrap();
  assert_eq!(second.resolve_calls, 0, "warm build must skip dependency resolution");
  assert_eq!(second.load_calls, 0, "warm build must skip the load hooks");
  assert_eq!(second.transform_calls, 0, "warm build must skip the transform hooks");
  assert_eq!(second.code, first.code, "cached build must produce identical output");
}

#[tokio::test(flavor = "multi_thread")]
async fn changing_cache_key_invalidates_entries() {
  let (cwd, cache_dir) = fresh_fixture("key");

  let first = Box::pin(build(&cwd, &cache_dir, Some("config-hash-a"))).await.unwrap();
  assert_eq!(first.transform_calls, 2);

  let same_key = Box::pin(build(&cwd, &cache_dir, Some("config-hash-a"))).await.unwrap();
  assert_eq!(same_key.transform_calls, 0, "same key must hit the cache");

  let new_key = Box::pin(build(&cwd, &cache_dir, Some("config-hash-b"))).await.unwrap();
  assert_eq!(new_key.transform_calls, 2, "a new key must invalidate every entry");
}

#[tokio::test(flavor = "multi_thread")]
async fn editing_a_module_reruns_only_its_own_pipeline() {
  let (cwd, cache_dir) = fresh_fixture("edit");

  let first = Box::pin(build(&cwd, &cache_dir, None)).await.unwrap();
  assert_eq!(first.transform_calls, 2);

  std::fs::write(cwd.join("dep.js"), "export const dep = '__MARKER__ dep edited';\n").unwrap();
  let after_edit = Box::pin(build(&cwd, &cache_dir, None)).await.unwrap();
  assert_eq!(after_edit.transform_calls, 1, "only the edited module must re-run its pipeline");
  assert_eq!(after_edit.load_calls, 1, "the unchanged importer must stay cached");
  assert!(after_edit.code.contains("dep edited"), "output must pick up the edit");

  let warm_again = Box::pin(build(&cwd, &cache_dir, None)).await.unwrap();
  assert_eq!(warm_again.transform_calls, 0, "the edited module must be cached again");
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_cached_dependency_falls_back_to_fresh_resolution() {
  let (cwd, cache_dir) = fresh_fixture("delete");

  let first = Box::pin(build(&cwd, &cache_dir, None)).await.unwrap();
  assert_eq!(first.transform_calls, 2);

  std::fs::remove_file(cwd.join("dep.js")).unwrap();
  let result = Box::pin(build(&cwd, &cache_dir, None)).await;
  assert!(
    result.is_err(),
    "a deleted dependency must fail like a cold build instead of replaying stale resolutions"
  );
}
