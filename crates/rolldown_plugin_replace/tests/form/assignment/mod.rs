use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

// doesn't replace lvalue in assignment
#[tokio::test(flavor = "multi_thread")]
async fn assignment() {
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
      values: [
        ("process.env.DEBUG".to_string(), "replaced".to_string()),
        ("hello".to_string(), "world".to_string()),
      ]
      .into_iter()
      .collect(),
      prevent_assignment: true,
      sourcemap: true,
      ..Default::default()
    }))],
  )
  .await;
}
