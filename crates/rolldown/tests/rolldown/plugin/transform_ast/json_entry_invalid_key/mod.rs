use std::sync::Arc;

use rolldown::{BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn preserve_string_named_exports_from_transformed_json_entry() {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("data".into()), import: "./data.json".into() }]),
        preserve_entry_signatures: Some(PreserveEntrySignatures::Strict),
        ..Default::default()
      },
      vec![Arc::new(JsonTransformAstPlugin::new(JsonMutation::PrependStaticImport))],
    )
    .await;
}
