use rolldown::{Bundler, BundlerOptions, InputItem, IsExternal, SourceMapType};
use rolldown_testing::workspace;
use sugar_path::SugarPath;

#[tokio::main]
async fn main() {
  let root = workspace::crate_dir("rolldown");
  let cwd = root.join("./examples").normalize();
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("react-dom".to_string()), import: "./index.js".to_string() },
      // InputItem { name: Some("react".to_string()), import: "react".to_string() },
    ]),
    external: Some(IsExternal::from_vec(vec!["test".to_string()])),
    cwd: cwd.into(),
    sourcemap: Some(SourceMapType::File),
    treeshake: rolldown::TreeshakeOptions::Option(rolldown::InnerOptions {
      module_side_effects: rolldown::ModuleSideEffects::Boolean(true),
    }),
    ..Default::default()
  });

  let result = bundler.write().await.unwrap();
  assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
}
