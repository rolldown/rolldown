use std::sync::Arc;

use rolldown::BundlerOptions;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn preserve_a_moved_payload_after_another_expression() {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions::default(),
      vec![Arc::new(JsonTransformAstPlugin::new(JsonMutation::MovePayloadAfterExpression))],
    )
    .await;
}
