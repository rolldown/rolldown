use rolldown_common::{Chunk, ChunkKind, OutputFormat, SymbolRef, WrapKind};
use rolldown_rstr::Rstr;

use crate::{stages::link_stage::LinkStageOutput, SharedOptions};

pub fn render_chunk_exports(
  this: &Chunk,
  graph: &LinkStageOutput,
  output_options: &SharedOptions,
) -> Option<String> {
  if let ChunkKind::EntryPoint { module: entry_module_id, .. } = &this.kind {
    let linking_info = &graph.metas[*entry_module_id];
    if matches!(linking_info.wrap_kind, WrapKind::Cjs) {
      match output_options.format {
        OutputFormat::Esm => {
          let wrap_ref_name =
            &this.canonical_names.get(&linking_info.wrapper_ref.unwrap()).unwrap_or_else(|| {
              panic!(
                "Cannot find canonical name for wrap ref {:?} of {:?}",
                linking_info.wrapper_ref.unwrap(),
                graph.module_table.normal_modules[*entry_module_id].resource_id
              )
            });
          return Some(format!("export default {wrap_ref_name}();\n"));
        }
        OutputFormat::Cjs => {
          unreachable!("entry CJS should not be wrapped in `OutputFormat::Cjs`")
        }
      }
    }
  }

  let export_items = get_export_items(this, graph);

  if export_items.is_empty() {
    return None;
  }
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
      } else {
        format!("{canonical_name} as {exported_name}")
      }
    })
    .collect::<Vec<_>>();
  s.push_str(&format!("export {{ {} }};", rendered_items.join(", "),));
  Some(s)
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
  output_options: &SharedOptions,
) -> Vec<String> {
  if let ChunkKind::EntryPoint { module: entry_module_id, .. } = &this.kind {
    let linking_info = &graph.metas[*entry_module_id];
    if matches!(linking_info.wrap_kind, WrapKind::Cjs) {
      match output_options.format {
        OutputFormat::Esm => {
          return vec!["default".to_string()];
        }
        OutputFormat::Cjs => {
          unreachable!("entry CJS should not be wrapped in `OutputFormat::Cjs`")
        }
      }
    }
  }

  get_export_items(this, graph)
    .into_iter()
    .map(|(exported_name, _)| exported_name.to_string())
    .collect::<Vec<_>>()
}
