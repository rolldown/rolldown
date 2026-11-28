use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::SourceMapType;
use rolldown_testing::{
  manual_integration_test,
  test_config::{DevTestMeta, TestMeta},
};

use super::failing_patch::fail_on_marker;

/// - Step 0 edits `hmr.js`. The client pulls the patch, and rendering it fails, so the client
///   reloads. The full build for the reloading page fails too: the sourcemap of the chunk also
///   has `hmr.js`, which still has the marker.
/// - Step 1 removes the marker. The full build succeeds, and the reload that is still pending
///   loads its output, which has `v3`.
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
