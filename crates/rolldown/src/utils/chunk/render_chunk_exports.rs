use crate::{
  stages::link_stage::LinkStageOutput,
  types::{
    generator::GenerateContext,
    symbol_ref_db::{SymbolRefDb, SymbolRefFlags},
  },
};
use std::borrow::Cow;

use rolldown_common::{
  Chunk, ChunkKind, ExportsKind, IndexModules, NormalizedBundlerOptions, OutputExports,
  OutputFormat, SymbolRef, WrapKind,
};
use rolldown_rstr::Rstr;
use rolldown_utils::ecma_script::{is_validate_identifier_name, property_access_str};

#[allow(clippy::too_many_lines)]
pub fn render_chunk_exports(
  ctx: &mut GenerateContext<'_>,
  export_mode: Option<&OutputExports>,
) -> Option<String> {
  let GenerateContext { chunk, link_output, options, .. } = ctx;
  let export_items = get_export_items(chunk, link_output);

  if export_items.is_empty() {
    return None;
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
      Some(s)
    }
    OutputFormat::Cjs | OutputFormat::Iife => {
      let mut s = String::new();
      match chunk.kind {
        ChunkKind::EntryPoint { module, .. } => {
          let module =
            &link_output.module_table.modules[module].as_normal().expect("should be normal module");
          if matches!(module.exports_kind, ExportsKind::Esm) {
            let rendered_items = export_items
              .into_iter()
              .map(|(exported_name, export_ref)| {
                let canonical_ref = link_output.symbols.par_canonical_ref_for(export_ref);
                let symbol = link_output.symbols.get(canonical_ref);
                let mut canonical_name = Cow::Borrowed(&chunk.canonical_names[&canonical_ref]);
                let exported_value = if let Some(ns_alias) = &symbol.namespace_alias {
                  let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
                  let property_name = &ns_alias.property_name;
                  Cow::Owned(format!("{canonical_ns_name}.{property_name}").into())
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
                  };
                  canonical_name.clone()
                };

                match export_mode {
                  Some(OutputExports::Named) => {
                    if must_keep_live_binding(
                      export_ref,
                      &link_output.symbols,
                      options,
                      &link_output.module_table.modules,
                    ) {
                      format!(
                        "Object.defineProperty(exports, '{exported_name}', {{
  enumerable: true,
  get: function () {{
    return {exported_value};
  }}
}});"
                      )
                    } else {
                      format!(
                        "{left_value} = {exported_value}",
                        left_value = property_access_str("exports", &exported_name)
                      )
                    }
                  }
                  Some(OutputExports::Default) => {
                    if matches!(options.format, OutputFormat::Cjs) {
                      format!("module.exports = {canonical_name};")
                    } else {
                      format!("return {canonical_name};")
                    }
                  }
                  Some(OutputExports::None) => String::new(),
                  _ => unreachable!(),
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
}});\n"
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

      Some(s)
    }
    OutputFormat::App => None,
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

fn must_keep_live_binding(
  export_ref: SymbolRef,
  symbol_ref_db: &SymbolRefDb,
  options: &NormalizedBundlerOptions,
  modules: &IndexModules,
) -> bool {
  if options.experimental.is_disable_live_bindings_enabled() {
    return false;
  }

  let canonical_ref = symbol_ref_db.par_canonical_ref_for(export_ref);

  let canonical_ref_flags = symbol_ref_db.get_flags(canonical_ref);

  if let Some(flags) = canonical_ref_flags {
    if flags.intersects(SymbolRefFlags::IS_CONST | SymbolRefFlags::IS_NOT_REASSIGNED) {
      return false;
    }
  }

  if !options.external_live_bindings && export_ref.is_created_by_import_from_external(modules) {
    return false;
  }

  true
}
