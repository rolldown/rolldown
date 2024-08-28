use rolldown::{BundlerOptions, InputItem};

use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn should_failed_to_resolve_the_external_module_with_diagnostic() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_error: true, ..Default::default() })
    .run(BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      cwd: Some(cwd),
      ..Default::default()
    })
    .await;
}
