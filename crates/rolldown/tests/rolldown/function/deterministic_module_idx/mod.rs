use std::{borrow::Cow, time::Duration};

use rolldown::{BundleFactory, BundleFactoryOptions, BundlerOptions, InputItem};
use rolldown_common::{BundleMode, ScanMode};
use rolldown_plugin::{HookLoadArgs, HookLoadReturn, HookUsage, Plugin, SharedLoadPluginContext};

const FIXTURE_ROOT: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/deterministic_module_idx");

/// The two intermediate importers, in the order they finish by default.
///
/// Indices are handed out while an importer's completion is handled, so it is the order these
/// two finish in - not the order their children finish in - that decides how the children are
/// interleaved in the module table.
const IMPORTERS: [&str; 2] = ["p.js", "q.js"];

/// Finishes the two importers in a controlled order.
///
/// Module tasks normally race, so which one finishes first is up to the scheduler. This makes
/// that order explicit and reversible, which is what lets the test tell "index assignment
/// follows arrival order" apart from "index assignment is stable".
#[derive(Debug)]
struct StaggerLoad {
  reversed: bool,
}

impl Plugin for StaggerLoad {
  fn name(&self) -> Cow<'static, str> {
    "stagger-load".into()
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if let Some(position) = IMPORTERS.iter().position(|importer| args.id.ends_with(importer)) {
      let step = if self.reversed { IMPORTERS.len() - 1 - position } else { position };
      tokio::time::sleep(Duration::from_millis(50 * (step as u64 + 1))).await;
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }
}

/// Module ids in `ModuleIdx` order.
async fn scan_module_ids(reversed: bool) -> Vec<String> {
  let mut factory = BundleFactory::new(BundleFactoryOptions {
    bundler_options: BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("main".to_string()),
        import: "./main.js".to_string(),
      }]),
      cwd: Some(FIXTURE_ROOT.into()),
      ..Default::default()
    },
    plugins: vec![Plugin::new_shared(StaggerLoad { reversed })],
    session: None,
    disable_tracing_setup: true,
  })
  .expect("failed to create bundle factory");

  let mut bundle =
    factory.create_bundle(BundleMode::FullBuild, None).expect("failed to create bundle");

  bundle
    .scan_modules(ScanMode::Full)
    .await
    .expect("scan should succeed")
    .module_table
    .modules
    .iter()
    .map(|module| module.id().to_string())
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn module_idx_assignment_is_independent_of_task_completion_order() {
  // `ModuleIdx` is handed out while handling task-completion messages. If that order is taken
  // as-is, reversing which importer finishes first interleaves their children differently, and
  // every link-stage pass that walks the module table by index sees a different graph for the
  // same input.
  let forward = scan_module_ids(false).await;
  let reversed = scan_module_ids(true).await;

  assert_eq!(
    forward, reversed,
    "module table order changed when the importers finished in the opposite order"
  );
}
