use std::sync::Arc;
use rolldown::{BundlerOptions, InputItem};
use rolldown_testing::{test_config::TestMeta, manual_integration_test};

#[tokio::test(flavor = "multi_thread")]
async fn import_chain() {
  manual_integration_test!()
    .build(TestMeta { expect_error: true, ..Default::default() })
    .run(BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("main".to_string()),
        import: "./main.js".to_string(),
      }]),
      ..Default::default()
    })
    .await;
}
