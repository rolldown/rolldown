use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType, TreeshakeOptions};
use rolldown_testing::workspace;
use sugar_path::SugarPath;

// cargo run --example basic

#[tokio::main]
async fn main() {
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("react-dom".to_string()), import: "./index.js".to_string() },
      // InputItem { name: Some("react".to_string()), import: "react".to_string() },
      InputItem { name: Some("react-dom".to_string()), import: "react-dom".to_string() },
      InputItem { name: Some("react".to_string()), import: "react".to_string() },
      "./entry.js".to_string().into(),
      InputItem { import: "./other-entry.js".to_string(), ..Default::default() },
      InputItem { name: Some("third-entry".to_string()), import: "./third-entry.js".to_string() },
    ]),
    cwd: cwd.into(),
    treeshake: rolldown::TreeshakeOptions::Option(rolldown::TreeshakeInnerOptions {
      module_side_effects: rolldown::ModuleSideEffects::Boolean(true),
    }),
    cwd: cwd.into(),
    cwd: Some(workspace::crate_dir("rolldown").join("./examples/basic").normalize()),
    sourcemap: Some(SourceMapType::File),
    ..Default::default()
  });

  let result = bundler.write().await.unwrap();
  assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
}
