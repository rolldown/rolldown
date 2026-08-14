use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use sugar_path::SugarPath;

/// Watches `side.txt` from `buildStart`, like a plugin watching a config file.
/// The file ends up in `watch_files` but belongs to no module and is no
/// transform dependency, so a change to it maps to zero changed modules.
#[derive(Debug)]
struct WatchSideFilePlugin;

impl Plugin for WatchSideFilePlugin {
  fn name(&self) -> Cow<'static, str> {
    "watch-side-file".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.add_watch_file(ctx.cwd().join("side.txt").normalize().to_str().unwrap());
    Ok(())
  }
}

/// Regression test: an update whose changed paths map to no module must still
/// retry `pending_rescans`, so the recovery latch (`last_task_errored`) stays
/// set while a broken file is unresolved.
///
/// - Step 0 breaks `dep.js`: the failed scan merges nothing, queues the file
///   in `pending_rescans`, and sets the latch.
/// - Step 1 touches `side.txt` (watched, maps to no module). The step must
///   re-fetch `dep.js` and fail again, keeping the latch set — not return an
///   empty `Noop` success that clears it.
/// - Step 2 restores `dep.js` to its exact pre-break bytes. The cache still
///   holds the pre-break AST, so the rebuilt output is byte-identical and only
///   the latch keeps the unchanged-output suppression off. The recovery patch
///   must ship, or clients stuck on the error overlay never leave it.
#[tokio::test(flavor = "multi_thread")]
async fn retry_pending_rescans_on_empty_update() {
  manual_integration_test!()
    // Deliberately no `ensure_latest_build_output_for_each_step`: forcing a
    // rebuild after every step would fail on the still-broken file at step 1
    // and re-set the latch, hiding the hole. Real dev defers the rebuild to
    // stale access, so the step-1 task is the HMR compute alone.
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./main.js".to_string(),
        }]),
        experimental: Some(ExperimentalOptions {
          dev_mode: Some(DevModeOptions::default()),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Arc::new(WatchSideFilePlugin)],
    )
    .await;
}
