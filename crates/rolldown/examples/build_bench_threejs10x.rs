use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_workspace as workspace;

// cargo run --example build_bench_threejs10x

#[tokio::main]
async fn main() {
  // Make sure that you have already run `just setup-bench`
  let root = workspace::root_dir();
  let project_root = workspace::crate_dir("rolldown");
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("threejs10x".to_string()),
      import: root.join("tmp/bench/three10x/entry.js").to_str().unwrap().to_string(),
    }]),
    cwd: Some(project_root.join("examples")),
    sourcemap: Some(SourceMapType::File),
    ..Default::default()
  });

  let _result = bundler.write().await.unwrap();
}
