use string_wizard::MagicString;

use crate::bundler::graph::graph::Graph;

use super::chunk::Chunk;

impl Chunk {
  pub fn render_exports_for_esm(&self, graph: &Graph) -> Option<MagicString<'static>> {
    if self.exports_to_other_chunks.is_empty() {
      return None;
    }
    let mut s = MagicString::new("");
    let mut export_items = self
      .exports_to_other_chunks
      .iter()
      .map(|(export_ref, alias)| {
        let canonical_ref = graph.symbols.par_get_canonical_ref(*export_ref);
        let canonical_name = &self.canonical_names[&canonical_ref];
        if canonical_name == alias {
          format!("{canonical_name}")
        } else {
          format!("{canonical_name} as {alias}")
        }
      })
      .collect::<Vec<_>>();
    export_items.sort();
    s.append(format!("export {{ {} }};", export_items.join(", "),));
    Some(s)
  }
}
