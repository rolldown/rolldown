use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{
  abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta, utils::create_fx_hash_map,
};

// matches most specific variables
#[tokio::test(flavor = "multi_thread")]
async fn match_variables() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta {
    expect_executed: false,
    visualize_sourcemap: true,
    ..Default::default()
  })
  .run_with_plugins(
    BundlerOptions {
      input: Some(vec!["./input.js".to_string().into()]),
      cwd: Some(cwd),
      ..Default::default()
    },
    vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
      values: create_fx_hash_map([
        ("BUILD".to_string(), "beta".to_string()),
        ("BUILD_VERSION".to_string(), "1.0.0".to_string()),
      ]),
      sourcemap: true,
      ..Default::default()
    }))],
  )
  .await;
}
