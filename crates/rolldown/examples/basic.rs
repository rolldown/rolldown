use rolldown::{Bundler, BundlerOptions, InputItem, IsExternal, SourceMapType};
use rolldown_testing::workspace;
use sugar_path::SugarPath;

#[tokio::main]
async fn main() {
  let root = workspace::crate_dir("rolldown");
  let cwd = root.join("./examples").normalize();
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("react-dom".to_string()), import: "./entry.js".to_string() },
      // InputItem { name: Some("react".to_string()), import: "react".to_string() },
    ]),
    // format: Some(rolldown::OutputFormat::Cjs),
    cwd: cwd.into(),
    sourcemap: Some(SourceMapType::File),
    // external: Some(IsExternal::from_vec(vec!["bar".to_string(), "foo".to_string()])),
    ..Default::default()
  });

  let result = bundler.write().await.unwrap();
  assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
}
