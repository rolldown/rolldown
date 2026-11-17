use std::collections::BTreeSet;

use rolldown::{Bundler, BundlerOptions, ExperimentalOptions, InputItem};
use rolldown_common::Output;

fn entry(name: &str) -> InputItem {
  InputItem { name: Some(name.to_string()), import: format!("{name}.js") }
}

#[tokio::test(flavor = "multi_thread")]
async fn chunk_graph_only_preserves_module_membership_and_omits_code() {
  let cwd = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/chunk_graph_only");

  async fn build(
    cwd: &str,
    chunk_graph_only: bool,
  ) -> Vec<(String, bool, BTreeSet<String>, BTreeSet<String>, usize)> {
    let mut bundler = Bundler::new(BundlerOptions {
      input: Some(vec![entry("a"), entry("b")]),
      cwd: Some(cwd.into()),
      experimental: Some(ExperimentalOptions {
        chunk_graph_only: Some(chunk_graph_only),
        ..Default::default()
      }),
      ..Default::default()
    })
    .expect("bundler");
    let output = bundler.generate().await.expect("generate");
    let mut chunks = output
      .assets
      .into_iter()
      .filter_map(|o| match o {
        Output::Chunk(c) => Some(c),
        Output::Asset(_) => None,
      })
      .collect::<Vec<_>>();
    chunks.sort_by(|a, b| a.name.cmp(&b.name));
    chunks
      .into_iter()
      .map(|c| {
        let module_ids = c.module_ids.iter().map(|id| id.to_string()).collect::<BTreeSet<_>>();
        let imports = c.imports.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>();
        (c.name.to_string(), c.is_entry, module_ids, imports, c.code.len())
      })
      .collect()
  }

  let full = build(cwd, false).await;
  let graph = build(cwd, true).await;

  assert!(!full.is_empty());
  assert_eq!(full.len(), graph.len(), "chunk count must match");
  for (full_chunk, graph_chunk) in full.iter().zip(graph.iter()) {
    assert_eq!(full_chunk.0, graph_chunk.0, "chunk name");
    assert_eq!(full_chunk.1, graph_chunk.1, "is_entry");
    assert_eq!(full_chunk.2, graph_chunk.2, "module ids");
    // Import edges: graph-only filenames are hashed from empty code, so the
    // basenames differ, but each entry must import the same number of chunks.
    assert_eq!(
      full_chunk.3.len(),
      graph_chunk.3.len(),
      "static import count for chunk {}",
      full_chunk.0
    );
    assert!(full_chunk.4 > 0, "full build should emit code for {}", full_chunk.0);
    assert_eq!(graph_chunk.4, 0, "chunkGraphOnly must leave code empty");
  }
}
