use std::path::PathBuf;

use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_common::Output;

const FIXTURE_ROOT: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/external_chunk_imports");

async fn chunk_imports(preserve_modules: bool) -> Vec<(String, Vec<String>)> {
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(
      ["a", "b", "c"]
        .into_iter()
        .map(|name| InputItem { name: Some(name.to_string()), import: format!("./{name}.js") })
        .collect(),
    ),
    cwd: Some(PathBuf::from(FIXTURE_ROOT)),
    external: Some(vec!["side-a".to_string(), "side-b".to_string()].into()),
    preserve_modules: Some(preserve_modules),
    ..Default::default()
  })
  .expect("failed to create bundler");
  let mut chunks = bundler
    .generate()
    .await
    .expect("build should succeed")
    .assets
    .into_iter()
    .filter_map(|asset| match asset {
      Output::Chunk(chunk) => Some((
        chunk.filename.to_string(),
        chunk.imports.iter().map(ToString::to_string).collect::<Vec<_>>(),
      )),
      Output::Asset(_) => None,
    })
    .collect::<Vec<_>>();
  chunks.sort();
  chunks
}

#[tokio::test(flavor = "multi_thread")]
async fn chunk_imports_list_each_external_once_in_source_order() {
  for preserve_modules in [false, true] {
    assert_eq!(
      chunk_imports(preserve_modules).await,
      vec![
        ("a.js".to_string(), vec!["side-a".to_string(), "side-b".to_string()]),
        ("b.js".to_string(), vec!["side-b".to_string(), "side-a".to_string()]),
        ("c.js".to_string(), vec!["side-b".to_string(), "side-a".to_string()]),
      ],
      "preserve_modules: {preserve_modules}",
    );
  }
}
