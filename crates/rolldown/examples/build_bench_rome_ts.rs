use rolldown::{Bundler, BundlerOptions, InputItem, ResolveOptions, SourceMapType};
use rolldown_workspace as workspace;

// cargo run --example build_bench_rome_ts

#[tokio::main]
async fn main() {
  // Make sure that you have already run `just setup-bench`
  let root = workspace::root_dir();
  let project_root = workspace::crate_dir("rolldown");
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("rome-ts".to_string()),
      import: root.join("tmp/bench/rome/src/entry.ts").to_str().unwrap().to_string(),
    }]),
    cwd: Some(project_root.join("examples")),
    sourcemap: Some(SourceMapType::File),
    // --- Required specific options for Rome
    shim_missing_exports: Some(true), // Need this due rome is not written with `isolatedModules: true`
    resolve: Some(ResolveOptions {
      tsconfig_filename: Some(
        root.join("tmp/bench/rome/src/tsconfig.json").to_str().unwrap().to_string(),
      ),
      ..Default::default()
    }),
    ..Default::default()
  });

  let _result = bundler.write().await.unwrap();
}
