use rolldown::{BundlerOptions, InjectImport, IsExternal};
use rolldown_plugin::Plugin;

use rolldown_plugin_inject::{InjectPlugin, InjectPluginOptions};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn jsx() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec!["./input.jsx".to_string().into()]),
        external: Some(IsExternal::from(vec![
          "./button.js".to_string(),
          "react/jsx-runtime".to_string(),
        ])),
        ..Default::default()
      },
      vec![Plugin::new_shared(InjectPlugin::new(InjectPluginOptions {
        injects: vec![InjectImport::named("Button".to_string(), None, "./button.js".to_string())],
        ..Default::default()
      }))],
    )
    .await;
}
