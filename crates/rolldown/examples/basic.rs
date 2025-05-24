use rolldown::{
  AdvancedChunksOptions, Bundler, BundlerOptions, InputItem, MatchGroup, SourceMapType,
};
use rolldown_utils::js_regex::HybridRegex;
use rolldown_workspace as workspace;
use sugar_path::SugarPath;

// cargo run --example basic

#[tokio::main]
async fn main() {
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![
      "./entry.js".to_string().into(),
      // InputItem { import: "./other-entry.js".to_string(), ..Default::default() },
      // InputItem { name: Some("third-entry".to_string()), import: "./third-entry.js".to_string() },
    ]),
    cwd: Some(workspace::crate_dir("rolldown").join("./examples/basic").normalize()),
    advanced_chunks: Some(AdvancedChunksOptions {
      groups: Some(vec![MatchGroup {
        name: "test".to_string(),
        test: Some(HybridRegex::new("").unwrap()),
        ..Default::default()
      }]),
      ..Default::default()
    }),
    sourcemap: Some(SourceMapType::File),
    ..Default::default()
  });

  let _result = bundler.write().await.unwrap();
}

// trigger
