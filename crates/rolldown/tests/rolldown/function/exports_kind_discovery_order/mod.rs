use std::{
  borrow::Cow,
  path::PathBuf,
  sync::atomic::{AtomicBool, Ordering},
};

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat};
use rolldown_common::Output;
use rolldown_plugin::{HookLoadArgs, HookLoadOutput, HookUsage, Plugin, SharedLoadPluginContext};
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
    const MANAGED: &[&str] =
      &["entry.js", "entry_swapped.js", "left.js", "right.js", "esm.js", "cjs.js", "init.js"];
    let file = Self::file_name(args.id);
    if !MANAGED.contains(&file) {
      return Ok(None);
    }
    // cross-branch barrier with no sleeps: the delayed aggregate waits for the
    // opposite leaf to finish loading, forcing opposite discovery orders.
    if self.first == "left" {
      if file == "right.js" && !self.leaf_done.load(Ordering::SeqCst) {
        self.notify.notified().await;
      }
    } else if file == "left.js" && !self.leaf_done.load(Ordering::SeqCst) {
      self.notify.notified().await;
    }
    let code = std::fs::read_to_string(args.id)?;
    if (self.first == "left" && file == "esm.js") || (self.first == "right" && file == "cjs.js") {
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
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem { name: Some("entry".to_string()), import: entry.to_string() }]),
      cwd: Some(PathBuf::from(FIXTURE_ROOT)),
      format: Some(OutputFormat::Iife),
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

#[tokio::test(flavor = "multi_thread")]
async fn exports_kind_stable_across_discovery_order() {
  let left_first = bundle_with_barrier("./entry.js", "left").await;
  let right_first = bundle_with_barrier("./entry.js", "right").await;
  assert_eq!(
    left_first, right_first,
    "iife output must be byte-identical across opposite discovery orders"
  );

  // interacting input: swapped source order stays deterministic as well.
  let swapped_left = bundle_with_barrier("./entry_swapped.js", "left").await;
  let swapped_right = bundle_with_barrier("./entry_swapped.js", "right").await;
  assert_eq!(
    swapped_left, swapped_right,
    "swapped entry must also be byte-identical across discovery orders"
  );
}
