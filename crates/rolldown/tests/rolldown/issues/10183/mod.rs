use std::path::PathBuf;

use rolldown::{Bundler, BundlerOptions, ChecksOptions, InputItem};

async fn invalid_annotation_warning_count(checks: Option<ChecksOptions>) -> usize {
  let project_dir =
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/issues/10183/project"));

  let mut bundler = Bundler::new(BundlerOptions {
    cwd: Some(project_dir),
    input: Some(vec![InputItem {
      name: Some("main".to_string()),
      import: "./main.js".to_string(),
    }]),
    checks,
    ..Default::default()
  })
  .expect("failed to create bundler");

  let output = bundler.generate().await.expect("build should succeed");
  output
    .warnings
    .iter()
    .filter(|warning| warning.kind().to_string() == "INVALID_ANNOTATION")
    .count()
}

#[tokio::test(flavor = "multi_thread")]
async fn suppresses_invalid_annotation_warning_outside_cwd() {
  assert_eq!(invalid_annotation_warning_count(None).await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_true_keeps_local_project_scope() {
  assert_eq!(
    invalid_annotation_warning_count(Some(ChecksOptions {
      invalid_annotation: Some(true),
      ..Default::default()
    }))
    .await,
    0
  );
}
