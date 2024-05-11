use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_testing::workspace;
use sugar_path::SugarPath;

#[tokio::main]
async fn main() {
  let root = workspace::crate_dir("rolldown");
  let cwd = root.join("./examples").normalize();
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![
      // InputItem { name: Some("react-dom".to_string()), import: "react-dom".to_string() },
      InputItem { name: Some("basic".to_string()), import: "./src/entry.js".to_string() },
    ]),
    cwd: cwd.into(),
    // sourcemap: Some(SourceMapType::File),
    ..Default::default()
  });

  let result = bundler.write().await.unwrap();
  assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
}
