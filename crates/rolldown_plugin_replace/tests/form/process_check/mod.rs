use std::sync::Arc;

use rolldown::{BundlerOptions, TreeshakeOptions};

use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

// Handles process type guards in replacements
#[tokio::test(flavor = "multi_thread")]
async fn process_check() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, visualize_sourcemap: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec!["./input.js".to_string().into()]),
        treeshake: TreeshakeOptions::Boolean(false),
        ..Default::default()
      },
      vec![Arc::new(ReplacePlugin::with_options(ReplaceOptions {
        values: std::iter::once(("process.env.NODE_ENV".to_string(), "\"production\"".to_string()))
          .collect(),
        prevent_assignment: true,
        sourcemap: true,
        object_guards: true,
        ..Default::default()
      }))],
    )
    .await;
}
