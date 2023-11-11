use rolldown_common::WrapKind;
use string_wizard::MagicString;

use crate::bundler::{
  graph::graph::Graph,
  options::{normalized_output_options::NormalizedOutputOptions, output_options},
};

use super::chunk::Chunk;

impl Chunk {
  pub fn render_exports(
    &self,
    graph: &Graph,
    output_options: &NormalizedOutputOptions,
  ) -> Option<MagicString<'static>> {
    if let Some(entry) = self.entry_module {
      let linking_info = &graph.linking_infos[entry];
      if matches!(linking_info.wrap_kind, WrapKind::Cjs) {
        match output_options.format {
          output_options::OutputFormat::Esm => {
            let wrap_ref_name = &self.canonical_names[&linking_info.wrap_ref.unwrap()];
            return Some(MagicString::new(format!("export default {wrap_ref_name}();\n")));
          }
        }
      }
    }

    let export_items = self.entry_module.map_or_else(
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
          .sorted_exports()
          .map(|(name, export)| (name, export.symbol_ref))
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
          let property_name = &ns_alias.property_name;
          s.append(format!("var {canonical_name} = {canonical_ns_name}.{property_name};\n"));
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
