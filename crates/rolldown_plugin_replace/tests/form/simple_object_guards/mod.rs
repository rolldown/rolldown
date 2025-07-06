use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

// Handles process type guards in replacements
#[tokio::test(flavor = "multi_thread")]
async fn simple_object_guards() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions { input: Some(vec!["./input.js".to_string().into()]), ..Default::default() },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: std::iter::once(("foo".to_string(), "bar".to_string())).collect(),
        object_guards: true,
        ..Default::default()
      }))],
    )
    .await;
}
