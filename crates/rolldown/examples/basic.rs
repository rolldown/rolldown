use rolldown::{Bundler, InputItem, InputOptions, OutputOptions};
use std::path::PathBuf;
use sugar_path::SugarPathBuf;

#[tokio::main]
async fn main() {
  rolldown_tracing::try_init_tracing();
  let root = PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").unwrap());
  let cwd = root.join("./examples").into_normalize();
  let mut bundler = Bundler::new(InputOptions {
    input: vec![
      InputItem { name: Some("react-dom".to_string()), import: "react-dom".to_string() },
      InputItem { name: Some("react".to_string()), import: "react".to_string() },
    ],
    cwd,
    ..Default::default()
  });

  let _outputs = bundler.write(OutputOptions::default()).await.unwrap();
  // println!("{outputs:#?}");
}
