use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_error::BatchedBuildDiagnostic;
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{
  manual_integration_test,
  test_config::{DevTestMeta, TestMeta},
};

/// When this string is found in any generated chunk, the plugin below fails its
/// `generate_bundle` hook. `dep.hmr-0.js` embeds it so the first HMR rebuild
/// fails; `dep.hmr-1.js` drops it so the next rebuild succeeds.
const FAIL_MARKER: &[u8] = b"FAIL_GENERATE_BUNDLE";

/// Fails the `generate_bundle` hook whenever a generated chunk contains
/// `FAIL_MARKER`.
///
/// `generate_bundle` runs inside `bundle_up`, *after* `create_output` has torn
/// the symbol-table scoping out of the long-lived scan cache. Failing here
/// reproduces a post-link dev-rebuild error that used to leave `ScanStageCache`
/// broken (either emptied or with torn scoping) and panic the next HMR cycle.
#[derive(Debug)]
struct FailGenerateBundleOnMarkerPlugin;

impl Plugin for FailGenerateBundleOnMarkerPlugin {
  fn name(&self) -> Cow<'static, str> {
    "fail-generate-bundle-on-marker".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::GenerateBundle
  }

  async fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    let should_fail = args.bundle.iter().any(|output| {
      output.content_as_bytes().windows(FAIL_MARKER.len()).any(|window| window == FAIL_MARKER)
    });
    if should_fail {
      Err(BatchedBuildDiagnostic::new(vec![
        anyhow::anyhow!("simulated post-link generate_bundle failure").into(),
      ]))?;
    }
    Ok(())
  }
}

/// Regression test: a dev rebuild that fails *after* the link stage must leave
/// the scan cache whole, so the next HMR cycle recovers instead of panicking.
///
/// - Initial build succeeds and populates `ScanStageCache`.
/// - HMR step 0 edits `dep.js` to embed `FAIL_MARKER`; the incremental rebuild
///   fails in the `generate_bundle` hook (post-link).
/// - HMR step 1 edits `dep.js` back to a clean state; the incremental rebuild
///   must succeed.
///
/// Before the fix, the failed step-0 rebuild left the cache either empty
/// (`with_cached_bundle` `?`-bailed and dropped it) or with torn symbol-table
/// scoping (`merge_immutable_fields_for_cache` was skipped on the error path),
/// and step 1 panicked in `get_snapshot()` / `oxc_semantic`.
#[tokio::test(flavor = "multi_thread")]
async fn recover_after_generate_bundle_error() {
  manual_integration_test!()
    .build(TestMeta {
      expect_executed: false,
      dev: DevTestMeta { ensure_latest_build_output_for_each_step: true, ..Default::default() },
      ..Default::default()
    })
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
      vec![Arc::new(FailGenerateBundleOnMarkerPlugin)],
    )
    .await;
}
