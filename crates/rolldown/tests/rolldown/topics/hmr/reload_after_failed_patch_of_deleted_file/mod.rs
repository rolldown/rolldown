use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::SourceMapType;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::failing_patch::fail_first_patch;

/// - Step 0 edits `child.js`. The client pulls the patch, and rendering it fails, so the client
///   reloads. It runs the output of the next full build, which has the edit.
/// - Step 1 removes the import of `child.js` from `parent.js` and deletes `child.js`. It is a hot
///   update of the reloaded client: the patch has no factory of the deleted file.
/// - Step 2 edits `parent.js` again, another hot update.
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
      sourcemap_path_transform: Some(fail_first_patch()),
      experimental: Some(ExperimentalOptions {
        dev_mode: Some(DevModeOptions::default()),
        ..Default::default()
      }),
      ..Default::default()
    })
    .await;
}
