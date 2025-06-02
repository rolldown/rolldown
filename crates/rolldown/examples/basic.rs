use rolldown::{Bundler, BundlerOptions, PreserveEntrySignatures};
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
    // format: Some(rolldown::OutputFormat::Cjs),
    cwd: Some(workspace::crate_dir("rolldown").join("./examples/basic").normalize()),
    // sourcemap: Some(SourceMapType::File),
    preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
    ..Default::default()
  });

  let _result = bundler.write().await.unwrap();
}

// trigger
