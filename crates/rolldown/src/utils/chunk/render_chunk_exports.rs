use std::borrow::Cow;
use std::fmt::Write as _;

use itertools::Itertools;
use rolldown_common::{
  Chunk, ChunkKind, ExportsKind, IndexModules, ModuleIdx, NormalizedBundlerOptions, OutputExports,
  OutputFormat, SymbolRef, SymbolRefDb, WrapKind,
};
use rolldown_rstr::Rstr;
use rolldown_utils::{
  concat_string,
  ecmascript::{property_access_str, to_module_import_export_name},
  indexmap::FxIndexSet,
};
use rustc_hash::FxHashSet;

use crate::{stages::link_stage::LinkStageOutput, types::generator::GenerateContext};

pub fn render_wrapped_entry_chunk(
  ctx: &GenerateContext<'_>,
  export_mode: Option<&OutputExports>,
) -> Option<String> {
  if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
    let entry_meta = &ctx.link_output.metas[entry_id];
    match entry_meta.wrap_kind {
      WrapKind::Esm => {
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        // init_xxx
        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          *wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );
        if entry_meta.is_tla_or_contains_tla_dependency {
          Some(concat_string!("await ", wrapper_ref_name, "();"))
        } else {
          Some(concat_string!(wrapper_ref_name, "();"))
        }
      }
      WrapKind::Cjs => {
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();

        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          *wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );

        match ctx.options.format {
          OutputFormat::Esm => {
            // export default require_xxx();
            Some(concat_string!("export default ", wrapper_ref_name.as_str(), "();\n"))
          }
          OutputFormat::Cjs => {
            if matches!(&export_mode, Some(OutputExports::Named)) {
              Some(render_object_define_property(
                "default",
                &concat_string!(wrapper_ref_name, "()"),
              ))
            } else {
              // module.exports = require_xxx();
              Some(concat_string!("module.exports = ", wrapper_ref_name, "();\n"))
            }
          }
          OutputFormat::Iife | OutputFormat::Umd => {
            if matches!(&export_mode, Some(OutputExports::Named)) {
              Some(render_object_define_property(
                "default",
                &concat_string!(wrapper_ref_name, "()"),
              ))
            } else {
              // return require_xxx();
              Some(concat_string!("return ", wrapper_ref_name, "();\n"))
            }
          }
        }
      }
      WrapKind::None => None,
    }
  } else {
    None
  }
}

#[allow(clippy::too_many_lines)]
pub fn render_chunk_exports(
  ctx: &GenerateContext<'_>,
  export_mode: Option<&OutputExports>,
) -> Option<String> {
  let GenerateContext { chunk, link_output, options, .. } = ctx;
  let export_items: Vec<(Rstr, SymbolRef)> = ctx.render_export_items_index_vec[ctx.chunk_idx]
    .clone()
    .into_iter()
    .flat_map(|(symbol_ref, names)| names.into_iter().map(|name| (name, symbol_ref)).collect_vec())
    .collect();

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
          let canonical_name = &chunk.canonical_names.get(&canonical_ref).unwrap_or_else(|| {
            panic!(
              "Canonical name not found for {:?} in chunk {:?} kind: {:?} for name {}",
              canonical_ref, chunk.name, chunk.kind, exported_name
            )
          });
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

          if canonical_name == &&exported_name {
            Cow::Borrowed(canonical_name.as_str())
          } else {
            Cow::Owned(concat_string!(
              canonical_name,
              " as ",
              to_module_import_export_name(&exported_name)
            ))
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
            &link_output.module_table[module].as_normal().expect("should be normal module");
          if matches!(module.exports_kind, ExportsKind::Esm) {
            let rendered_items = export_items
              .into_iter()
              .map(|(exported_name, export_ref)| {
                let canonical_ref = link_output.symbol_db.canonical_ref_for(export_ref);
                let exported_value = ctx.finalized_string_pattern_for_symbol_ref(
                  canonical_ref,
                  ctx.chunk_idx,
                  &chunk.canonical_names,
                );

                match export_mode {
                  Some(OutputExports::Named) => {
                    if must_keep_live_binding(
                      export_ref,
                      &link_output.symbol_db,
                      options,
                      &link_output.module_table.modules,
                    ) {
                      render_object_define_property(&exported_name, &exported_value)
                    } else {
                      concat_string!(
                        property_access_str("exports", exported_name.as_str()),
                        " = ",
                        exported_value.as_str(),
                        ";"
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

          // Track already imported external modules to avoid duplicates
          // First check if any of these external modules have already been imported elsewhere in the chunk
          let mut imported_external_modules: FxHashSet<SymbolRef> = ctx
            .chunk
            .direct_imports_from_external_modules
            .iter()
            .map(|(idx, _)| {
              let external = &ctx.link_output.module_table[*idx]
                .as_external()
                .expect("Should be external module here");
              external.namespace_ref
            })
            .collect();

          external_modules.iter().for_each(|idx| {
          let external = &ctx.link_output.module_table[*idx].as_external().expect("Should be external module here");
          let binding_ref_name =
          &ctx.chunk.canonical_names[&external.namespace_ref];
            let import_stmt =
"Object.keys($NAME).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return $NAME[k]; }
  });
});\n".replace("$NAME", binding_ref_name);

          s.push('\n');
          // Only generate require statement if this external module hasn't been imported yet
          if imported_external_modules.insert(external.namespace_ref) {
            writeln!(s, "var {} = require(\"{}\");", binding_ref_name, &external.get_import_path(chunk)).unwrap();
          }
          s.push_str(&import_stmt);
        });
        }
        ChunkKind::Common => {
          let rendered_items = export_items
            .into_iter()
            .map(|(exported_name, export_ref)| {
              let canonical_ref = link_output.symbol_db.canonical_ref_for(export_ref);
              let symbol = link_output.symbol_db.get(canonical_ref);
              let canonical_name = &chunk.canonical_names[&canonical_ref];

              match &symbol.namespace_alias {
                Some(ns_alias) => {
                  let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
                  let property_name = &ns_alias.property_name;
                  render_object_define_property(
                    &exported_name,
                    &concat_string!(canonical_ns_name, ".", property_name),
                  )
                }
                _ => render_object_define_property(&exported_name, canonical_name),
              }
            })
            .collect::<Vec<_>>();
          s.push_str(&rendered_items.join("\n"));
        }
      }

      if s.is_empty() {
        return None;
      }
      Some(s)
    }
  }
}

#[inline]
pub fn render_object_define_property(key: &str, value: &str) -> String {
  concat_string!(
    "Object.defineProperty(exports, '",
    key,
    "', {
  enumerable: true,
  get: function () {
    return ",
    value,
    ";
  }
});"
  )
}

pub fn get_export_items(chunk: &Chunk) -> Vec<(Rstr, SymbolRef)> {
  let mut export_items = chunk
    .exports_to_other_chunks
    .iter()
    .flat_map(|(export_ref, alias_list)| {
      alias_list.iter().map(|alias| (alias.clone(), *export_ref))
    })
    .collect::<Vec<_>>();

  export_items.sort_unstable_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

  export_items
}

pub fn get_chunk_export_names(chunk: &Chunk, graph: &LinkStageOutput) -> Vec<Rstr> {
  if let ChunkKind::EntryPoint { module: entry_id, .. } = &chunk.kind {
    let entry_meta = &graph.metas[*entry_id];
    if matches!(entry_meta.wrap_kind, WrapKind::Cjs) {
      return vec![Rstr::new("default")];
    }
  }

  get_export_items(chunk).into_iter().map(|(exported_name, _)| exported_name).collect::<Vec<_>>()
}

pub fn get_chunk_export_names_with_ctx(ctx: &GenerateContext<'_>) -> Vec<Rstr> {
  let GenerateContext { chunk, link_output, render_export_items_index_vec, .. } = ctx;
  if let ChunkKind::EntryPoint { module: entry_id, .. } = &chunk.kind {
    let entry_meta = &link_output.metas[*entry_id];
    if matches!(entry_meta.wrap_kind, WrapKind::Cjs) {
      return vec![Rstr::new("default")];
    }
  }
  render_export_items_index_vec[ctx.chunk_idx].values().flatten().cloned().collect::<Vec<_>>()
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
