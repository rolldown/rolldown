use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::SourceMapType;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::failing_patch::fail_first_patch;

/// - Step 0 edits `hmr.js`: it now imports `new-static.js` and calls `import('./new-dynamic.js')`.
///   Both modules are new to the graph. The client pulls the patch, and rendering it fails, so the
///   client reloads. It runs the output of the next full build, which has the edit.
/// - Step 1 edits only `other.js`. It is a hot update of the reloaded client.
#[tokio::test(flavor = "multi_thread")]
async fn reload_after_failed_patch() {
  manual_integration_test!()
    .build(TestMeta::default())
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
