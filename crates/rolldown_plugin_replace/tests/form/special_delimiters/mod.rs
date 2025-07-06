use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

// allows delimiters with special characters
#[tokio::test(flavor = "multi_thread")]
async fn special_delimiters() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, visualize_sourcemap: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions { input: Some(vec!["./input.js".to_string().into()]), ..Default::default() },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: std::iter::once(("special".to_string(), "replaced".to_string())).collect(),
        delimiters: Some(("\\b".to_string(), "\\b".to_string())),
        sourcemap: true,
        ..Default::default()
      }))],
    )
    .await;
}
