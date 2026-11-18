use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::SourceMapType;
use rolldown_testing::{
  manual_integration_test,
  test_config::{DevTestMeta, TestMeta},
};

use super::failing_patch::fail_first_patch;

/// - Step 0 edits `hmr.js`. Rendering the patch fails after the edit was merged into the module
///   graph, so no client receives it.
/// - A full build runs, like the one Vite runs when a page loads after the failure. Vite then
///   reloads every tab, so the client runs the new output, which has the edit of step 0.
/// - Step 1 edits only `other.js`. It must be a hot update, not another full reload: the full
///   build already delivered the lost edit.
#[tokio::test(flavor = "multi_thread")]
async fn no_reload_after_full_build() {
  manual_integration_test!()
    .build(TestMeta {
      dev: DevTestMeta { full_build_after_steps: vec![0], ..Default::default() },
      ..Default::default()
    })
    .run(BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("main".to_string()),
        import: "./main.js".to_string(),
      }]),
      sourcemap: Some(SourceMapType::File),
      sourcemap_path_transform: Some(fail_first_patch()),
      experimental: Some(ExperimentalOptions {
        dev_mode: Some(DevModeOptions::default()),
        ..Default::default()
      }),
      ..Default::default()
    })
    .await;
}
