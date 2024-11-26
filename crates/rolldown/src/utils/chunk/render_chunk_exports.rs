use crate::{stages::link_stage::LinkStageOutput, types::generator::GenerateContext};
use std::borrow::Cow;

use rolldown_common::{
  Chunk, ChunkKind, EntryPointKind, ExportsKind, IndexModules, ModuleIdx, NormalizedBundlerOptions,
  OutputExports, OutputFormat, SymbolRef, SymbolRefDb, WrapKind,
};
use rolldown_rstr::Rstr;
use rolldown_utils::{
  concat_string,
  ecmascript::{is_validate_identifier_name, property_access_str},
  indexmap::FxIndexSet,
};

#[allow(clippy::too_many_lines)]
pub fn render_chunk_exports(
  ctx: &GenerateContext<'_>,
  export_mode: Option<&OutputExports>,
) -> Option<String> {
  let GenerateContext { chunk, link_output, options, .. } = ctx;
  let export_items = get_export_items(chunk, link_output);

  match options.format {
    OutputFormat::Esm => {
      if export_items.is_empty() {
        return None;
      }
      let mut s = String::new();
      let rendered_items = export_items
        .into_iter()
        .map(|(exported_name, export_ref)| {
          let canonical_ref = link_output.symbol_db.canonical_ref_for(export_ref);
          let symbol = link_output.symbol_db.get(canonical_ref);
          let canonical_name = &chunk.canonical_names[&canonical_ref];
          if let Some(ns_alias) = &symbol.namespace_alias {
            let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
            let property_name = &ns_alias.property_name;
            s.push_str(&concat_string!(
              "var ",
              canonical_name,
              " = ",
              canonical_ns_name,
              ".",
              property_name,
              ";\n"
            ));
          }

          if canonical_name == &exported_name {
            Cow::Borrowed(canonical_name.as_str())
          } else if is_validate_identifier_name(&exported_name) {
            Cow::Owned(concat_string!(canonical_name, " as ", exported_name))
          } else {
            Cow::Owned(concat_string!(canonical_name, " as '", exported_name, "'"))
          }
        })
        .collect::<Vec<_>>();
      s.push_str(&concat_string!("export { ", rendered_items.join(", "), " };"));
      Some(s)
    }
    OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd => {
      let mut s = String::new();
      match chunk.kind {
        ChunkKind::EntryPoint { module, .. } => {
          let module =
            &link_output.module_table.modules[module].as_normal().expect("should be normal module");
          if matches!(module.exports_kind, ExportsKind::Esm) {
            let rendered_items = export_items
              .into_iter()
              .map(|(exported_name, export_ref)| {
                let canonical_ref = link_output.symbol_db.canonical_ref_for(export_ref);
                let symbol = link_output.symbol_db.get(canonical_ref);
                let mut canonical_name = Cow::Borrowed(&chunk.canonical_names[&canonical_ref]);
                let exported_value = if let Some(ns_alias) = &symbol.namespace_alias {
                  let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
                  let property_name = &ns_alias.property_name;
                  Cow::Owned(property_access_str(canonical_ns_name, property_name).into())
                } else if link_output.module_table.modules[canonical_ref.owner].is_external() {
                  let namespace = &chunk.canonical_names[&canonical_ref];
                  Cow::Owned(namespace.as_str().into())
                } else {
                  let cur_chunk_idx = ctx.chunk_idx;
                  let canonical_ref_owner_chunk_idx =
                    link_output.symbol_db.get(canonical_ref).chunk_id.unwrap();
                  let is_this_symbol_point_to_other_chunk =
                    cur_chunk_idx != canonical_ref_owner_chunk_idx;
                  if is_this_symbol_point_to_other_chunk {
                    let require_binding = &ctx.chunk.require_binding_names_for_other_chunks
                      [&canonical_ref_owner_chunk_idx];
                    canonical_name = Cow::Owned(Rstr::new(&concat_string!(
                      require_binding,
                      ".",
                      canonical_name.as_str()
                    )));
                  };
                  canonical_name.clone()
                };

                match export_mode {
                  Some(OutputExports::Named) => {
                    if must_keep_live_binding(
                      export_ref,
                      &link_output.symbol_db,
                      options,
                      &link_output.module_table.modules,
                    ) {
                      concat_string!(
                        "Object.defineProperty(exports, '",
                        exported_name.as_str(),
                        "', {
  enumerable: true,
  get: function () {
    return ",
                        exported_value.as_str(),
                        ";
  }
});"
                      )
                    } else {
                      concat_string!(
                        property_access_str("exports", exported_name.as_str()),
                        " = ",
                        exported_value.as_str()
                      )
                    }
                  }
                  Some(OutputExports::Default) => {
                    if matches!(options.format, OutputFormat::Cjs) {
                      concat_string!("module.exports = ", exported_value.as_str(), ";")
                    } else {
                      concat_string!("return ", exported_value.as_str(), ";")
                    }
                  }
                  Some(OutputExports::None) => String::new(),
                  _ => unreachable!(),
                }
              })
              .collect::<Vec<_>>();
            s.push_str(&rendered_items.join("\n"));
          }

          let meta = &ctx.link_output.metas[module.idx];
          let external_modules = meta
            .star_exports_from_external_modules
            .iter()
            .map(|rec_idx| module.ecma_view.import_records[*rec_idx].resolved_module)
            .collect::<FxIndexSet<ModuleIdx>>();
          external_modules.iter().for_each(|idx| {
          let external = &ctx.link_output.module_table.modules[*idx].as_external().expect("Should be external module here");
          let binding_ref_name =
          &ctx.chunk.canonical_names[&external.namespace_ref];
            let import_stmt =
"Object.keys($NAME).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return $NAME[k]; }
  });
});\n".replace("$NAME", binding_ref_name);

          s.push_str(&format!("\nvar {} = require(\"{}\");\n", binding_ref_name, &external.name));
          s.push_str(&import_stmt);
        });
        }
        ChunkKind::Common => {
          export_items.into_iter().for_each(|(exported_name, export_ref)| {
            let canonical_ref = link_output.symbol_db.canonical_ref_for(export_ref);
            let symbol = link_output.symbol_db.get(canonical_ref);
            let canonical_name = &chunk.canonical_names[&canonical_ref];

            if let Some(ns_alias) = &symbol.namespace_alias {
              let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
              let property_name = &ns_alias.property_name;
              s.push_str(&concat_string!(
                "Object.defineProperty(exports, '",
                exported_name,
                "', {
  enumerable: true,
  get: function () {
    return ",
                canonical_ns_name,
                ".",
                property_name,
                ";\n  }
});\n"
              ));
            } else {
              s.push_str(&concat_string!(
                "Object.defineProperty(exports, '",
                exported_name,
                "', {
  enumerable: true,
  get: function () {
    return ",
                canonical_name,
                ";
  }
});"
              ));
            };
          });
        }
      }

      if s.is_empty() {
        return None;
      }
      Some(s)
    }
    OutputFormat::App => None,
  }
}

pub fn get_export_items(chunk: &Chunk, graph: &LinkStageOutput) -> Vec<(Rstr, SymbolRef)> {
  match chunk.kind {
    ChunkKind::EntryPoint { module, is_user_defined, .. } => {
      let meta = &graph.metas[module];
      meta
        .referenced_canonical_exports_symbols(
          module,
          if is_user_defined { EntryPointKind::UserDefined } else { EntryPointKind::DynamicImport },
          &graph.dynamic_import_exports_usage_map,
        )
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
  symbol_db: &SymbolRefDb,
  options: &NormalizedBundlerOptions,
  modules: &IndexModules,
) -> bool {
  if options.experimental.is_disable_live_bindings_enabled() {
    return false;
  }

  let canonical_ref = symbol_db.canonical_ref_for(export_ref);

  if canonical_ref.is_declared_by_const(symbol_db).unwrap_or(false) {
    // For unknown case, we consider it as not declared by `const`.
    return false;
  }

  if canonical_ref.is_not_reassigned(symbol_db).unwrap_or(false) {
    // For unknown case, we consider it as reassigned.
    return false;
  }

  if !options.external_live_bindings
    && canonical_ref.is_created_by_import_stmt_that_target_external(symbol_db, modules)
  {
    return false;
  }

  true
}
