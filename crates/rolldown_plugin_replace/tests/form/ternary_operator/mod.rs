use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

// replaces value inside ternary operators
#[tokio::test(flavor = "multi_thread")]
async fn ternary_operator() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, visualize_sourcemap: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions { input: Some(vec!["./input.js".to_string().into()]), ..Default::default() },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: [
          ("condition".to_string(), "first".to_string()),
          ("exprIfTrue".to_string(), "second".to_string()),
          ("exprIfFalse".to_string(), "third".to_string()),
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
