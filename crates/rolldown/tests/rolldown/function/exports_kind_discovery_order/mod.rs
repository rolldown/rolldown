use std::{
  borrow::Cow,
  path::PathBuf,
  process::Command,
  sync::atomic::{AtomicBool, Ordering},
};

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{ExperimentalOptions, ModuleType, Output, Platform};
use rolldown_plugin::{HookLoadArgs, HookLoadOutput, HookUsage, Plugin, SharedLoadPluginContext};
use rustc_hash::FxHashMap;
use tokio::sync::Notify;

const FIXTURE_ROOT: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/exports_kind_discovery_order");

#[derive(Debug)]
struct BarrierPlugin {
  first: &'static str,
  leaf_done: AtomicBool,
  notify: Notify,
}

impl BarrierPlugin {
  fn file_name(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id).rsplit('\\').next().unwrap_or(id)
  }
}

impl Plugin for BarrierPlugin {
  fn name(&self) -> Cow<'static, str> {
    "discovery-order-barrier".into()
  }

  async fn load(
    &self,
    _ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    let file = Self::file_name(args.id);
    const MANAGED: &[&str] = &[
      "entry.js",
      "entry_swapped.js",
      "left.js",
      "right.js",
      "esm.js",
      "cjs.js",
      "init.js",
      "setup.js",
    ];
    if !MANAGED.contains(&file) {
      return Ok(None);
    }
    // Cross-branch barrier with no sleeps: the delayed aggregate waits for the
    // opposite leaf to finish loading, forcing opposite discovery orders.
    if self.first == "left" {
      if file == "right.js" && !self.leaf_done.load(Ordering::SeqCst) {
        self.notify.notified().await;
      }
    } else if file == "left.js" && !self.leaf_done.load(Ordering::SeqCst) {
      self.notify.notified().await;
    }
    let code = tokio::fs::read_to_string(args.id).await?;
    if (self.first == "left" && file == "esm.js")
      || (self.first == "right" && file == "cjs.js")
    {
      self.leaf_done.store(true, Ordering::SeqCst);
      self.notify.notify_one();
    }
    Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }
}

async fn bundle_with_barrier(entry: &str, first: &'static str) -> String {
  let mut module_types = FxHashMap::default();
  module_types.insert(".js".to_string(), ModuleType::Jsx);
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: entry.to_string(),
      }]),
      cwd: Some(PathBuf::from(FIXTURE_ROOT)),
      platform: Some(Platform::Neutral),
      format: Some(OutputFormat::Iife),
      module_types: Some(module_types),
      experimental: Some(ExperimentalOptions { lazy_barrel: Some(false), ..Default::default() }),
      ..Default::default()
    },
    vec![Plugin::new_shared(BarrierPlugin {
      first,
      leaf_done: AtomicBool::new(false),
      notify: Notify::new(),
    })],
  )
  .expect("failed to create bundler");
  let assets = bundler.generate().await.expect("build should succeed").assets;
  let mut chunks: Vec<String> = assets
    .into_iter()
    .filter_map(|asset| match asset {
      Output::Chunk(chunk) => Some(chunk.code.clone()),
      Output::Asset(_) => None,
    })
    .collect();
  assert_eq!(chunks.len(), 1, "iife should emit a single chunk");
  chunks.pop().unwrap()
}

fn assert_setup_runs_once(code: &str, tag: &str) {
  let output_dir =
    std::env::temp_dir().join(format!("rolldown-exports-kind-{}-{tag}", std::process::id()));
  let _ = std::fs::remove_dir_all(&output_dir);
  std::fs::create_dir_all(&output_dir).expect("temp dir should be created");
  std::fs::write(output_dir.join("bundle.js"), code).expect("bundle should be written");
  let output = Command::new("node")
    .current_dir(&output_dir)
    .args([
      "--input-type=module",
      "--eval",
      "import fs from 'node:fs'; import vm from 'node:vm'; \
       const code = fs.readFileSync('./bundle.js', 'utf8'); \
       const ctx = vm.createContext({}); vm.runInContext(code, ctx); \
       if (ctx.answer !== 1) { throw new Error('setup must execute exactly once, answer=' + ctx.answer); }",
    ])
    .output()
    .expect("Node.js should execute the bundle");
  assert!(
    output.status.success(),
    "bundle {tag} must execute setup exactly once\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr),
  );
  let _ = std::fs::remove_dir_all(&output_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn exports_kind_stable_across_discovery_order() {
  let left_first = bundle_with_barrier("./entry.js", "left").await;
  let right_first = bundle_with_barrier("./entry.js", "right").await;
  assert_eq!(
    left_first, right_first,
    "iife output must be byte-identical across opposite discovery orders"
  );
  assert_setup_runs_once(&left_first, "entry-left");
  assert_setup_runs_once(&right_first, "entry-right");

  // Interacting input: swapped source order stays deterministic as well.
  let swapped_left = bundle_with_barrier("./entry_swapped.js", "left").await;
  let swapped_right = bundle_with_barrier("./entry_swapped.js", "right").await;
  assert_eq!(
    swapped_left, swapped_right,
    "swapped entry must also be byte-identical across discovery orders"
  );
  assert_setup_runs_once(&swapped_left, "swapped-left");
  assert_setup_runs_once(&swapped_right, "swapped-right");
}
