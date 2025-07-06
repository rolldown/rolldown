use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

// allows delimiters with special characters
#[tokio::test(flavor = "multi_thread")]
async fn special_delimiters() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(
    TestMeta { expect_executed: false, visualize_sourcemap: true, ..Default::default() },
    abs_file_dir!(),
  )
  .run_with_plugins(
    BundlerOptions {
      input: Some(vec!["./input.js".to_string().into()]),
      cwd: Some(cwd),
      ..Default::default()
    },
    vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
      values: std::iter::once(("special".to_string(), "replaced".to_string())).collect(),
      delimiters: Some(("\\b".to_string(), "\\b".to_string())),
      sourcemap: true,
      ..Default::default()
    }))],
  )
  .await;
}
