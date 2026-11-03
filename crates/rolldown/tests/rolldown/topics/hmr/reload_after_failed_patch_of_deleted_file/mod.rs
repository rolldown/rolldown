use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::SourceMapType;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::failing_patch::fail_on_marker;

/// - Step 0 edits `child.js`. Rendering the patch fails after the edit was merged into the
///   module graph, so no client receives it.
/// - Step 1 removes the import of `child.js` from `parent.js` and deletes `child.js`. Every
///   client gets a full reload, and the full build succeeds without the deleted file.
/// - Step 2 edits `parent.js` again. It must be a hot update: the full build delivered the lost
///   edit, so nothing is lost anymore.
#[tokio::test(flavor = "multi_thread")]
async fn reload_after_failed_patch_of_deleted_file() {
  manual_integration_test!()
    .build(TestMeta::default())
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
