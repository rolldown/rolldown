use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::ReplacePlugin;
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn replace_strings() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_executed: false, ..Default::default() }, abs_file_dir!())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec!["./input.js".to_string().into()]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(ReplacePlugin::new(
        [
          ("ANSWER".to_string(), "42".to_string()),
          ("typeof window".to_string(), "\"object\"".to_string()),
        ]
        .into_iter()
        .collect(),
      ))],
    )
    .await;
}
