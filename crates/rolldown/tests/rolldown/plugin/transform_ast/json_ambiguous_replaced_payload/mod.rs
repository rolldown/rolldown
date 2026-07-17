use std::sync::Arc;

use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_error::EventKind;

use super::{JsonMutation, JsonTransformAstPlugin};

#[tokio::test(flavor = "multi_thread")]
async fn reject_a_parenthesized_impostor_after_whole_statement_replacement() {
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      cwd: Some(rolldown_testing::abs_file_dir!()),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      ..Default::default()
    },
    vec![Arc::new(JsonTransformAstPlugin::new(
      JsonMutation::ClonePayloadAndAppendParenthesizedExpression,
    ))],
  )
  .expect("bundler");

  let Err(errors) = bundler.generate().await else { panic!("ambiguous payload must fail") };
  assert!(errors.iter().any(|diagnostic| matches!(diagnostic.kind(), EventKind::TransformError)));
  assert!(
    errors.iter().any(|diagnostic| diagnostic.id().is_some_and(|id| id.ends_with("data.json")))
  );
  let rendered = errors.to_string();
  assert!(rendered.contains("data.json"));
  assert!(rendered.contains("multiple payload candidates after identity was lost"));
}
