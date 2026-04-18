use rolldown::BundlerOptions;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn startup_entry_shared_module_not_eager_chunked() {
  Box::pin(
    manual_integration_test!()
      .build(TestMeta { expect_executed: true, expect_warning: Some(false), ..Default::default() })
      .run(BundlerOptions::default()),
  )
  .await;
}
