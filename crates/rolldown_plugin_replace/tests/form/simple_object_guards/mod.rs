use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

// Handles process type guards in replacements
#[tokio::test(flavor = "multi_thread")]
async fn simple_object_guards() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_executed: false, ..Default::default() }, abs_file_dir!())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec!["./input.js".to_string().into()]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: std::iter::once(("foo".to_string(), "bar".to_string())).collect(),
        object_guards: true,
        ..Default::default()
      }))],
    )
    .await;
}
