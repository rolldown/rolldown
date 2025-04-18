use std::sync::Arc;

use rolldown::{BundlerOptions, InputItem};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn test() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_error: false, ..Default::default() })
    .run(BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "entry.js".to_string(),
      }]),
      cwd: Some(cwd),
      chunk_filenames: Some(rolldown::ChunkFilenamesOutputOption::Fn(Arc::new(|chunk| {
        let name = format!("{}.js", chunk.name);
        Box::pin(async move { Ok(name) })
      }))),
      ..Default::default()
    })
    .await;
}
