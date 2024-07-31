use std::borrow::Cow;

use rolldown_common::{
  Chunk, ChunkKind, EcmaModule, ExportsKind, NormalizedBundlerOptions, OutputExports, OutputFormat,
  SymbolRef, WrapKind,
};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_rstr::Rstr;
use rolldown_utils::ecma_script::is_validate_identifier_name;

use crate::{stages::link_stage::LinkStageOutput, types::generator::GenerateContext};

#[allow(clippy::too_many_lines)]
pub fn render_chunk_exports(ctx: &mut GenerateContext<'_>) -> DiagnosableResult<Option<String>> {
  let GenerateContext { chunk, link_output, options, .. } = ctx;
  let export_items = get_export_items(chunk, link_output);

  if export_items.is_empty() {
    return Ok(None);
  }

  match options.format {
    OutputFormat::Esm => {
      let mut s = String::new();
      let rendered_items = export_items
        .into_iter()
        .map(|(exported_name, export_ref)| {
          let canonical_ref = link_output.symbols.par_canonical_ref_for(export_ref);
          let symbol = link_output.symbols.get(canonical_ref);
          let canonical_name = &chunk.canonical_names[&canonical_ref];
          if let Some(ns_alias) = &symbol.namespace_alias {
            let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
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
      Ok(Some(s))
    }
    OutputFormat::Cjs | OutputFormat::Iife => {
      let mut s = String::new();
      match chunk.kind {
        ChunkKind::EntryPoint { module, .. } => {
          let module =
            &link_output.module_table.modules[module].as_ecma().expect("should be ecma module");
          if matches!(module.exports_kind, ExportsKind::Esm) {
            let export_mode = determine_export_mode(&options.exports, module, &export_items)?;
            if matches!(export_mode, OutputExports::Named) {
              s.push_str("Object.defineProperty(exports, '__esModule', { value: true });\n");
            }
            let rendered_items = export_items
              .into_iter()
              .map(|(exported_name, export_ref)| {
                let canonical_ref = link_output.symbols.par_canonical_ref_for(export_ref);
                let symbol = link_output.symbols.get(canonical_ref);
                let mut canonical_name = Cow::Borrowed(&chunk.canonical_names[&canonical_ref]);
                if let Some(ns_alias) = &symbol.namespace_alias {
                  let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
                  let property_name = &ns_alias.property_name;
                  s.push_str(&format!(
                    "var {canonical_name} = {canonical_ns_name}.{property_name};\n"
                  ));
                } else {
                  let cur_chunk_idx = ctx.chunk_idx;
                  let canonical_ref_owner_chunk_idx =
                    link_output.symbols.get(canonical_ref).chunk_id.unwrap();
                  let is_this_symbol_point_to_other_chunk =
                    cur_chunk_idx != canonical_ref_owner_chunk_idx;
                  if is_this_symbol_point_to_other_chunk {
                    let require_binding = &ctx.chunk.require_binding_names_for_other_chunks
                      [&canonical_ref_owner_chunk_idx];
                    canonical_name =
                      Cow::Owned(Rstr::new(&format!("{require_binding}.{canonical_name}")));
                  }
                }

                match export_mode {
                  OutputExports::Named => {
                    format!(
                      "Object.defineProperty(exports, '{exported_name}', {{
  enumerable: true,
  get: function () {{
    return {canonical_name};
  }}
}});"
                    )
                  }
                  OutputExports::Default => {
                    if matches!(options.format, OutputFormat::Cjs) {
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
            let canonical_ref = link_output.symbols.par_canonical_ref_for(export_ref);
            let symbol = link_output.symbols.get(canonical_ref);
            let canonical_name = &chunk.canonical_names[&canonical_ref];

            if let Some(ns_alias) = &symbol.namespace_alias {
              let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
              let property_name = &ns_alias.property_name;
              s.push_str(&format!(
                "Object.defineProperty(exports, '{exported_name}', {{
  enumerable: true,
  get: function () {{
    return {canonical_ns_name}.{property_name};
  }}
}});"
              ));
            } else {
              s.push_str(&format!(
                "Object.defineProperty(exports, '{exported_name}', {{
  enumerable: true,
  get: function () {{
    return {canonical_name};
  }}
  }});"
              ));
            };
          });
        }
      }

      Ok(Some(s))
    }
    OutputFormat::App => Ok(None),
  }
}

pub fn get_export_items(chunk: &Chunk, graph: &LinkStageOutput) -> Vec<(Rstr, SymbolRef)> {
  match chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      let meta = &graph.metas[module];
      meta
        .canonical_exports()
        .map(|(name, export)| (name.clone(), export.symbol_ref))
        .collect::<Vec<_>>()
    }
    ChunkKind::Common => {
      let mut tmp = chunk
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
  chunk: &Chunk,
  graph: &LinkStageOutput,
  options: &NormalizedBundlerOptions,
) -> Vec<String> {
  if matches!(options.format, OutputFormat::Esm) {
    if let ChunkKind::EntryPoint { module: entry_id, .. } = &chunk.kind {
      let entry_meta = &graph.metas[*entry_id];
      if matches!(entry_meta.wrap_kind, WrapKind::Cjs) {
        return vec!["default".to_string()];
      }
    }
  }

  get_export_items(chunk, graph)
    .into_iter()
    .map(|(exported_name, _)| exported_name.to_string())
    .collect::<Vec<_>>()
}

// Port from https://github.com/rollup/rollup/blob/master/src/utils/getExportMode.ts
pub fn determine_export_mode(
  export_mode: &OutputExports,
  module: &EcmaModule,
  exports: &[(Rstr, SymbolRef)],
) -> DiagnosableResult<OutputExports> {
  match export_mode {
    OutputExports::Named => Ok(OutputExports::Named),
    OutputExports::Default => {
      if exports.len() != 1 || exports[0].0.as_str() != "default" {
        return Err(vec![BuildDiagnostic::invalid_export_option(
          "default".into(),
          module.stable_id.as_str().into(),
          exports.iter().map(|(name, _)| name.as_str().into()).collect(),
        )]);
      }
      Ok(OutputExports::Default)
    }
    OutputExports::None => {
      if !exports.is_empty() {
        return Err(vec![BuildDiagnostic::invalid_export_option(
          "none".into(),
          module.stable_id.as_str().into(),
          exports.iter().map(|(name, _)| name.as_str().into()).collect(),
        )]);
      }
      Ok(OutputExports::None)
    }
    OutputExports::Auto => {
      if exports.is_empty() {
        Ok(OutputExports::None)
      } else if exports.len() == 1 && exports[0].0.as_str() == "default" {
        Ok(OutputExports::Default)
      } else {
        // TODO add warnings
        Ok(OutputExports::Named)
      }
    }
  }
}
