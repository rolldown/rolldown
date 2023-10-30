use oxc::span::Atom;
use string_wizard::MagicString;

use crate::bundler::graph::{graph::Graph, linker::is_ambiguous_export};

use super::chunk::Chunk;

impl Chunk {
  pub fn render_exports_for_esm(&self, graph: &Graph) -> Option<MagicString<'static>> {
    let mut export_items = self.entry_module.map_or_else(
      || {
        self
          .exports_to_other_chunks
          .iter()
          .map(|(export_ref, alias)| (alias, *export_ref))
          .collect::<Vec<_>>()
      },
      |entry_module_id| {
        let linking_info = &graph.linking_infos[entry_module_id];
        linking_info
          .exclude_ambiguous_resolved_exports
          .iter()
          .map(|name| (name, linking_info.resolved_exports.get(name).unwrap().symbol_ref))
          .collect::<Vec<_>>()
      },
    );

    if export_items.is_empty() {
      return None;
    }
    let mut s = MagicString::new("");
    let rendered_items = export_items
      .into_iter()
      .map(|(exported_name, export_ref)| {
        let canonical_ref = graph.symbols.par_canonical_ref_for(export_ref);
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
