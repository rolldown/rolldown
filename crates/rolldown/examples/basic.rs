use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use std::path::PathBuf;
use sugar_path::SugarPathBuf;

#[tokio::main]
async fn main() {
  rolldown_tracing::try_init_tracing();
  let root = PathBuf::from(
    &std::env::var("CARGO_MANIFEST_DIR")
      .unwrap_or(std::env::current_dir().unwrap().display().to_string()),
  );
  let cwd = root.join("./examples").into_normalize();
  let mut bundler = Bundler::new(BundlerOptions {
    input: vec![
      InputItem { name: Some("react-dom".to_string()), import: "react-dom".to_string() },
      InputItem { name: Some("react".to_string()), import: "react".to_string() },
    ],
    cwd: cwd.into(),
    sourcemap: Some(SourceMapType::File),
    ..Default::default()
  });

  let _outputs = bundler.write().await.unwrap();
  // println!("{outputs:#?}");
}
