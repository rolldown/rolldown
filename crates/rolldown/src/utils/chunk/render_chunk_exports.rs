use rolldown_common::{Chunk, ChunkKind, OutputFormat, SymbolRef, WrapKind};
use rolldown_rstr::Rstr;
use rolldown_utils::ecma_script::is_validate_identifier_name;

use crate::{stages::link_stage::LinkStageOutput, SharedOptions};

pub fn render_chunk_exports(
  this: &Chunk,
  graph: &LinkStageOutput,
  output_options: &SharedOptions,
) -> Option<String> {
  let export_items = get_export_items(this, graph);

  if export_items.is_empty() {
    return None;
  }

  match output_options.format {
    OutputFormat::Esm => {
      let mut s = String::new();
      let rendered_items = export_items
        .into_iter()
        .map(|(exported_name, export_ref)| {
          let canonical_ref = graph.symbols.par_canonical_ref_for(export_ref);
          let symbol = graph.symbols.get(canonical_ref);
          let canonical_name = &this.canonical_names[&canonical_ref];
          if let Some(ns_alias) = &symbol.namespace_alias {
            let canonical_ns_name = &this.canonical_names[&ns_alias.namespace_ref];
            let property_name = &ns_alias.property_name;
            s.push_str(&format!("var {canonical_name} = {canonical_ns_name}.{property_name};\n"));
          }

          if canonical_name == &exported_name {
            format!("{canonical_name}")
          } else if is_validate_identifier_name(&exported_name) {
            format!("{canonical_name} as {exported_name}")
          } else {
            format!("{canonical_name} as '{exported_name}'")
          }
        })
        .collect::<Vec<_>>();
      s.push_str(&format!("export {{ {} }};", rendered_items.join(", "),));
      Some(s)
    }
    OutputFormat::Cjs => {
      let mut s = String::new();
      export_items.into_iter().for_each(|(exported_name, export_ref)| {
        let canonical_ref = graph.symbols.par_canonical_ref_for(export_ref);
        let symbol = graph.symbols.get(canonical_ref);
        let canonical_name = &this.canonical_names[&canonical_ref];
        let assignee_name = if is_validate_identifier_name(&exported_name) {
          format!("exports.{exported_name}")
        } else {
          format!("exports['{exported_name}']")
        };
        if let Some(ns_alias) = &symbol.namespace_alias {
          let canonical_ns_name = &this.canonical_names[&ns_alias.namespace_ref];
          let property_name = &ns_alias.property_name;
          s.push_str(&format!("{assignee_name} = {canonical_ns_name}.{property_name};;\n"));
        } else {
          s.push_str(&format!("{assignee_name} = {canonical_name};\n"));
        }
      });

      Some(s)
    }
  }
}

fn get_export_items(this: &Chunk, graph: &LinkStageOutput) -> Vec<(Rstr, SymbolRef)> {
  match this.kind {
    ChunkKind::EntryPoint { module, .. } => {
      let meta = &graph.metas[module];
      meta
        .canonical_exports()
        .map(|(name, export)| (name.clone(), export.symbol_ref))
        .collect::<Vec<_>>()
    }
    ChunkKind::Common => {
      let mut tmp = this
        .exports_to_other_chunks
        .iter()
        .map(|(export_ref, alias)| (alias.clone(), *export_ref))
        .collect::<Vec<_>>();

      tmp.sort_unstable_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

      tmp
    }
  }
}

pub fn get_chunk_export_names(
  this: &Chunk,
  graph: &LinkStageOutput,
  options: &SharedOptions,
) -> Vec<String> {
  if matches!(options.format, OutputFormat::Esm) {
    if let ChunkKind::EntryPoint { module: entry_id, .. } = &this.kind {
      let entry_meta = &graph.metas[*entry_id];
      if matches!(entry_meta.wrap_kind, WrapKind::Cjs) {
        return vec!["default".to_string()];
      }
    }
  }

  get_export_items(this, graph)
    .into_iter()
    .map(|(exported_name, _)| exported_name.to_string())
    .collect::<Vec<_>>()
}
