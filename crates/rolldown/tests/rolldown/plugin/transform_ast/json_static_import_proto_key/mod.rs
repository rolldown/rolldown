use std::sync::Arc;

use rolldown::BundlerOptions;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn preserve_proto_as_an_own_json_property() {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions::default(),
      vec![Arc::new(JsonTransformAstPlugin::new(JsonMutation::PrependStaticImport))],
    )
    .await;
}
