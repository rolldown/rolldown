use std::sync::Arc;

use rolldown::{BundlerOptions, InputItem};

use rolldown_plugin_virtual::{VirtualOption, VirtualPlugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn test_plugin_virtual() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { snapshot_skip_assets: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "src/entry.js".to_string(),
        }]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(VirtualPlugin::new(VirtualOption {
        modules: [
          ("batman".to_string(), "export default 'na na na na na'".to_string()),
          ("src/robin.js".to_string(), "export default 'batman'".to_string()),
        ]
        .into(),
      }))],
    )
    .await;
}
