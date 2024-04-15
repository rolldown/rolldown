use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use std::path::PathBuf;
use sugar_path::SugarPath;

#[tokio::main]
async fn main() {
  rolldown_tracing::try_init_tracing();
  let root = PathBuf::from(
    &std::env::var("CARGO_MANIFEST_DIR")
      .unwrap_or(std::env::current_dir().unwrap().display().to_string()),
  );
  let cwd = root.join("./examples").normalize();
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("react-dom".to_string()), import: "react-dom".to_string() },
      InputItem { name: Some("react".to_string()), import: "react".to_string() },
    ]),
    cwd: cwd.into(),
    sourcemap: Some(SourceMapType::File),
    ..Default::default()
  });

  let result = bundler.write().await.unwrap();
  assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
  // println!("{outputs:#?}");
}
