use std::sync::Arc;

use rolldown::{BundlerOptions, PreserveEntrySignatures};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn preserve_snapshot_initialization_across_an_esm_cycle() {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        preserve_modules: Some(true),
        preserve_entry_signatures: Some(PreserveEntrySignatures::Strict),
        ..Default::default()
      },
      vec![Arc::new(JsonTransformAstPlugin::new(JsonMutation::PrependStaticImport))],
    )
    .await;
}
