use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

// matches most specific variables
#[tokio::test(flavor = "multi_thread")]
async fn match_variables() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, visualize_sourcemap: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions { input: Some(vec!["./input.js".to_string().into()]), ..Default::default() },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: [
          ("BUILD".to_string(), "beta".to_string()),
          ("BUILD_VERSION".to_string(), "1.0.0".to_string()),
        ]
        .into_iter()
        .collect(),
        sourcemap: true,
        ..Default::default()
      }))],
    )
    .await;
}
