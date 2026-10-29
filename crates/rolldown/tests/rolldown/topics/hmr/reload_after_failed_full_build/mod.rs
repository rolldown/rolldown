use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::SourceMapType;
use rolldown_testing::{
  manual_integration_test,
  test_config::{DevTestMeta, TestMeta},
};

use super::failing_patch::fail_on_marker;

/// - Step 0 edits `hmr.js`. Rendering the patch fails after the edit was merged into the module
///   graph, so no client receives it.
/// - A full build runs, like the one Vite runs when a page loads after the failure. It fails
///   too: the sourcemap of the chunk also has `hmr.js`, which still has the marker.
/// - Step 1 removes the marker. The engine recovers with a full build, which succeeds. Every
///   client must get a full reload: no patch carries `v3`, and Vite reloads after a successful
///   build only when a reload is pending.
#[tokio::test(flavor = "multi_thread")]
async fn reload_after_failed_full_build() {
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
      sourcemap_path_transform: Some(fail_on_marker()),
      experimental: Some(ExperimentalOptions {
        dev_mode: Some(DevModeOptions::default()),
        ..Default::default()
      }),
      ..Default::default()
    })
    .await;
}
