use std::sync::Arc;

use rolldown::BundlerOptions;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn keep_named_exports_as_snapshots_when_default_is_mutated() {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions::default(),
      vec![Arc::new(JsonTransformAstPlugin::new(JsonMutation::PrependStaticImport))],
    )
    .await;
}
