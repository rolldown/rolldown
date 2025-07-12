use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin_replace::ReplacePlugin;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn replace_strings() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions { input: Some(vec!["./input.js".to_string().into()]), ..Default::default() },
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
