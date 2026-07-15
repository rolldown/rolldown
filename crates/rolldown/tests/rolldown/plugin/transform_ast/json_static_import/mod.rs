use std::sync::Arc;

use rolldown::BundlerOptions;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn preserve_static_import_before_json_payload() {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions::default(),
      vec![Arc::new(JsonTransformAstPlugin::new(JsonMutation::PrependStaticImport))],
    )
    .await;
}
