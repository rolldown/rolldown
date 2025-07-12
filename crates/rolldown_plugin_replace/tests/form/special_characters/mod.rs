use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

// supports special characters
#[tokio::test(flavor = "multi_thread")]
async fn special_characters() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, visualize_sourcemap: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions { input: Some(vec!["./input.js".to_string().into()]), ..Default::default() },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: std::iter::once(("require('one')".to_string(), "1".to_string())).collect(),
        delimiters: Some((String::new(), String::new())),
        sourcemap: true,
        ..Default::default()
      }))],
    )
    .await;
}
