use std::sync::Arc;

use rolldown::{BundlerOptions, OutputFormat, Platform};
use rolldown_testing::{
  integration_test::NamedBundlerOptions, manual_integration_test, test_config::TestMeta,
};

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn render_arbitrary_json_export_names_in_iife_and_umd() {
  let options = |format| BundlerOptions {
    format: Some(format),
    name: Some("JsonExports".into()),
    platform: Some(Platform::Browser),
    ..Default::default()
  };
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_multiple(
      vec![
        NamedBundlerOptions {
          description: Some("iife".into()),
          options: options(OutputFormat::Iife),
          snapshot: Some(false),
          config_name: Some("iife".into()),
          expect_execution_failure: None,
        },
        NamedBundlerOptions {
          description: Some("umd".into()),
          options: options(OutputFormat::Umd),
          snapshot: Some(false),
          config_name: Some("umd".into()),
          expect_execution_failure: None,
        },
      ],
      vec![Arc::new(JsonTransformAstPlugin::new(JsonMutation::PrependStaticImport))],
    )
    .await;
}
