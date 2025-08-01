use rolldown::{BundlerOptions, InputItem};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn test() {
  manual_integration_test!()
    .build(TestMeta::default())
    .run(BundlerOptions {
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      ..Default::default()
    })
    .await;
}
