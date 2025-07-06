use std::sync::Arc;

use rolldown::{BundlerOptions, TreeshakeOptions};

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

// Handles process type guards in replacements
#[tokio::test(flavor = "multi_thread")]
async fn process_check() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(
    TestMeta { expect_executed: false, visualize_sourcemap: true, ..Default::default() },
    abs_file_dir!(),
  )
  .run_with_plugins(
    BundlerOptions {
      input: Some(vec!["./input.js".to_string().into()]),
      treeshake: TreeshakeOptions::Boolean(false),
      cwd: Some(cwd),
      ..Default::default()
    },
    vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
      values: std::iter::once(("process.env.NODE_ENV".to_string(), "\"production\"".to_string()))
        .collect(),
      prevent_assignment: true,
      sourcemap: true,
      object_guards: true,
      ..Default::default()
    }))],
  )
  .await;
}
