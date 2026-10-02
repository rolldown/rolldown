use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::SourceMapType;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::failing_patch::fail_first_patch;

/// - Step 0 edits `hmr.js`: it now imports `new-static.js` and calls `import('./new-dynamic.js')`.
///   Both modules are new to the graph. Rendering the patch fails after the edit was merged into
///   the module graph, so no client receives it.
/// - Step 1 edits only `other.js`. A patch would not carry the edit of step 0, so every client
///   gets a full reload instead, and a full build runs. The reloaded client runs the new output,
///   which has both edits.
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
