use std::sync::Arc;

use rolldown::{BundlerOptions, TreeshakeOptions};

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

// Handles process type guards in replacements
#[tokio::test(flavor = "multi_thread")]
async fn process_check() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec!["./input.js".to_string().into()]),
        cwd: Some(cwd),
        treeshake: TreeshakeOptions::Boolean(false),
        ..Default::default()
      },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: [("process.env.NODE_ENV".to_string(), "\"production\"".to_string())].into(),
        prevent_assignment: true,
        object_guards: true,
        ..Default::default()
      }))],
    )
    .await;
}
