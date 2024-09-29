use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn replace_strings() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec!["./input.js".to_string().into()]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: [("original".to_string(), "replaced".to_string())].into(),
        delimiters: Some(("<%".to_string(), "%>".to_string())),
        ..Default::default()
      }))],
    )
    .await;
}
