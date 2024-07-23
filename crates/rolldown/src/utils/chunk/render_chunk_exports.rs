use rolldown_common::{
  Chunk, ChunkKind, ExportsKind, NormalizedBundlerOptions, OutputExports, OutputFormat, SymbolRef,
  WrapKind,
};
use rolldown_rstr::Rstr;
use rolldown_utils::ecma_script::is_validate_identifier_name;

use crate::{runtime::RuntimeModuleBrief, stages::link_stage::LinkStageOutput};

pub fn render_chunk_exports(
  this: &Chunk,
  _runtime: &RuntimeModuleBrief,
  graph: &LinkStageOutput,
  output_options: &NormalizedBundlerOptions,
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
    OutputFormat::Cjs | OutputFormat::Iife => {
      let mut s = String::new();
      match this.kind {
        ChunkKind::EntryPoint { module, .. } => {
          let module = &graph.module_table.modules[module].as_ecma().unwrap();
          if matches!(module.exports_kind, ExportsKind::Esm) {
            let export_mode = determine_export_mode(this, &output_options.exports, graph).unwrap();
            if matches!(export_mode, OutputExports::Named) {
              s.push_str("Object.defineProperty(exports, '__esModule', { value: true });\n");
            }
            let rendered_items = export_items
              .into_iter()
              .map(|(exported_name, export_ref)| {
                let canonical_ref = graph.symbols.par_canonical_ref_for(export_ref);
                let symbol = graph.symbols.get(canonical_ref);
                let canonical_name = &this.canonical_names[&canonical_ref];
                if let Some(ns_alias) = &symbol.namespace_alias {
                  let canonical_ns_name = &this.canonical_names[&ns_alias.namespace_ref];
                  let property_name = &ns_alias.property_name;
                  s.push_str(&format!(
                    "var {canonical_name} = {canonical_ns_name}.{property_name};\n"
                  ));
                }

                match export_mode {
                  OutputExports::Named => {
                    if is_validate_identifier_name(&exported_name) {
                      format!("exports.{exported_name} = {canonical_name};")
                    } else {
                      format!("exports['{exported_name}'] = {canonical_name};")
                    }
                  }
                  OutputExports::Default => {
                    if matches!(output_options.format, OutputFormat::Cjs) {
                      format!("module.exports = {canonical_name};")
                    } else {
                      format!("return {canonical_name};")
                    }
                  }
                  OutputExports::None => String::new(),
                  OutputExports::Auto => unreachable!(),
                }
              })
              .collect::<Vec<_>>();
            s.push_str(&rendered_items.join("\n"));
          }
        }
        ChunkKind::Common => {
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
        }
      }

      Some(s)
    }
    OutputFormat::App => None,
  }
}

pub fn get_export_items(this: &Chunk, graph: &LinkStageOutput) -> Vec<(Rstr, SymbolRef)> {
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
  options: &NormalizedBundlerOptions,
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

// Port from https://github.com/rollup/rollup/blob/master/src/utils/getExportMode.ts
pub fn determine_export_mode(
  this: &Chunk,
  export_mode: &OutputExports,
  graph: &LinkStageOutput,
) -> anyhow::Result<OutputExports> {
  let export_items = get_export_items(this, graph);

  match export_mode {
    OutputExports::Named => Ok(OutputExports::Named),
    OutputExports::Default => {
      if export_items.len() != 1 || export_items[0].0.as_str() != "default" {
        // TODO improve the backtrace
        anyhow::bail!(
          "Chunk was specified for `output.exports`, but entry module has invalid exports"
        );
      }
      Ok(OutputExports::Default)
    }
    OutputExports::None => {
      if !export_items.is_empty() {
        // TODO improve the backtrace
        anyhow::bail!(
          "Chunk was specified for `output.exports`, but entry module has invalid exports"
        );
      }
      Ok(OutputExports::None)
    }
    OutputExports::Auto => {
      if export_items.is_empty() {
        Ok(OutputExports::None)
      } else if export_items.len() == 1 && export_items[0].0.as_str() == "default" {
        Ok(OutputExports::Default)
      } else {
        // TODO add warnings
        Ok(OutputExports::Named)
      }
    }
  }
}
