use string_wizard::MagicString;

use crate::bundler::graph::graph::Graph;

use super::chunk::Chunk;

impl Chunk {
  pub fn render_exports_for_esm(&self, graph: &Graph) -> Option<MagicString<'static>> {
    let mut export_items = self.entry_module.map_or_else(
      || {
        self
          .exports_to_other_chunks
          .iter()
          .map(|(export_ref, alias)| (alias, export_ref))
          .collect::<Vec<_>>()
      },
      |entry_module_id| {
        let linking_info = &graph.linking_infos[entry_module_id];
        linking_info.resolved_exports.iter().collect::<Vec<_>>()
      },
    );

    export_items.sort_by_key(|(exported_name, _)| exported_name.as_str());
    if export_items.is_empty() {
      return None;
    }
    let mut s = MagicString::new("");
    let rendered_items = export_items
      .into_iter()
      .map(|(exported_name, export_ref)| {
        let canonical_ref = graph.symbols.par_get_canonical_ref(*export_ref);
        let symbol = graph.symbols.get(canonical_ref);
        let canonical_name = &self.canonical_names[&canonical_ref];
        if let Some(ns_alias) = &symbol.namespace_alias {
          let canonical_ns_name = &self.canonical_names[&ns_alias.namespace_ref];
          s.append(format!(
            "var {canonical_name} = {canonical_ns_name}.{};\n",
            ns_alias.property_name
          ));
        }
        if canonical_name == exported_name {
          format!("{canonical_name}")
        } else {
          format!("{canonical_name} as {exported_name}")
        }
      })
      .collect::<Vec<_>>();
    s.append(format!("export {{ {} }};", rendered_items.join(", "),));
    Some(s)
  }
}
