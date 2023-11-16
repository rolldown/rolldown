use miette::Result;
use rolldown::{Bundler, InputItem, InputOptions};
use std::path::PathBuf;
use sugar_path::SugarPathBuf;

#[tokio::main]
async fn main() -> Result<()> {
  let _guard = rolldown_tracing::try_init_tracing_with_chrome_layer();
  let root = PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").unwrap());
  let cwd = root.join("./examples").into_normalize();
  let mut bundler = Bundler::new(InputOptions {
    input: vec![InputItem { name: Some("basic".to_string()), import: "react-dom".to_string() }],
    cwd,
    ..Default::default()
  });

  let _outputs = bundler.write(Default::default()).await.unwrap();
  // println!("{outputs:#?}");

  Ok(())
}
