use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{
  abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta, utils::create_fx_hash_map,
};

// supports special characters
#[tokio::test(flavor = "multi_thread")]
async fn special_characters() {
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
      values: create_fx_hash_map([("require('one')".to_string(), "1".to_string())]),
      delimiters: Some((String::new(), String::new())),
      sourcemap: true,
      ..Default::default()
    }))],
  )
  .await;
}
