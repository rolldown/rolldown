use std::sync::Arc;

use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn render_string_named_json_exports_from_cjs_common_chunk() {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![
          InputItem { name: Some("a".into()), import: "./a.js".into() },
          InputItem { name: Some("b".into()), import: "./b.js".into() },
        ]),
        format: Some(OutputFormat::Cjs),
        ..Default::default()
      },
      vec![Arc::new(JsonTransformAstPlugin::new(JsonMutation::PrependStaticImport))],
    )
    .await;
}
