use string_wizard::MagicString;

use crate::bundler::{chunk_graph::ChunkGraph, graph::graph::Graph};

use super::chunk::Chunk;

impl Chunk {
  pub fn render_imports_for_esm(
    &self,
    graph: &Graph,
    chunk_graph: &ChunkGraph,
  ) -> MagicString<'static> {
    let mut s = MagicString::new("");
    self.imports_from_other_chunks.iter().for_each(|(chunk_id, items)| {
      let chunk = &chunk_graph.chunks[*chunk_id];
      let mut import_items = items
        .iter()
        .map(|item| {
          let imported = chunk
            .canonical_names
            .get(&graph.symbols.par_canonical_ref_for(item.import_ref))
            .cloned()
            .unwrap();
          let alias = item.export_alias.as_ref().unwrap();
          if imported == alias {
            format!("{imported}")
          } else {
            format!("{imported} as {alias}")
          }
        })
        .collect::<Vec<_>>();
      import_items.sort();
      s.append(format!(
        "import {{ {} }} from \"./{}\";\n",
        import_items.join(", "),
        chunk.file_name.as_ref().unwrap()
      ));
    });
    s
  }
}
