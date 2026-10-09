use arcstr::ArcStr;
use itertools::Itertools;
use rolldown_common::{
  AddonRenderContext, ChunkIdx, ExportsKind, ExternalModule, ImportKind, ImportRecordIdx,
  ModuleIdx, ModuleTable, RUNTIME_MODULE_KEY, Specifier, SymbolRef,
};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{concat_string, ecmascript::to_module_import_export_name};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ecmascript::ecma_generator::{RenderedModuleSource, RenderedModuleSources},
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::{render_chunk_exports, render_wrapped_entry_chunk},
};
use json_escape_simd::escape;

use super::{
  share_factory::render_inline_records,
  utils::{is_use_strict_directive, render_chunk_directives},
};

/// `carried_sources`: the inline common chunk records this file carries, finalized and printed
/// for it; empty unless this file carries a record.
#[expect(clippy::needless_pass_by_value)]
pub fn render_esm<'code>(
  ctx: &GenerateContext<'code>,
  addon_render_context: AddonRenderContext<'code>,
  module_sources: &'code RenderedModuleSources,
  carried_sources: &'code [(ChunkIdx, RenderedModuleSources)],
) -> SourceJoiner<'code> {
  let mut source_joiner = SourceJoiner::default();
  let AddonRenderContext { hashbang, banner, intro, outro, footer, directives } =
    addon_render_context;

  if let Some(hashbang) = hashbang {
    source_joiner.append_source(hashbang);
  }

  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/linker/linker.go#L5686-L5698
  if !directives.is_empty() {
    let rendered_chunk_directives =
      render_chunk_directives(directives.iter().filter(|d| !is_use_strict_directive(d)));
    if !rendered_chunk_directives.is_empty() {
      source_joiner.append_source(rendered_chunk_directives);
    }
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  if let Some(imports) = render_esm_chunk_imports(ctx) {
    source_joiner.append_source(imports);
  }

  // Registrations and bridges of inline common chunk records come right after the imports and
  // before anything of this file's own runs.
  render_inline_records(ctx, &mut source_joiner, carried_sources);

  // chunk content
  render_chunk_content(ctx, module_sources, &mut source_joiner);

  if let Some(source) = render_wrapped_entry_chunk(ctx, None) {
    source_joiner.append_source(source);
  }

  if let Some(exports) = render_chunk_exports(ctx, None) {
    source_joiner.append_source(exports);
  }

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  source_joiner
}

fn render_chunk_content<'code>(
  ctx: &GenerateContext<'_>,
  module_sources: &'code [RenderedModuleSource],
  source_joiner: &mut SourceJoiner<'code>,
) {
  // Dev mode: every chunk carries a graph-rows prelude for the client-side HMR walk.
  // It references `__rolldown_runtime__`, so in the chunk that defines the runtime it
  // must land right after the runtime module; everywhere else it comes first (the
  // global is guaranteed by the chunk's ESM import of the runtime-carrying chunk).
  let mut dev_graph_prelude = if ctx.options.is_dev_mode_enabled() {
    crate::hmr::module_graph_delta::render_register_graph_source(
      &ctx.link_output.module_table,
      ctx.chunk.modules.iter().copied(),
      None,
    )
  } else {
    None
  };
  // The per-module runtime checks below are reached through `take_if`, so they run only
  // while a prelude is still pending — never in production, and at most until the runtime
  // module is found in dev.
  let is_runtime_module =
    |idx: ModuleIdx| ctx.link_output.module_table[idx].id().as_str() == RUNTIME_MODULE_KEY;
  let chunk_carries_runtime =
    dev_graph_prelude.is_some() && ctx.chunk.modules.iter().copied().any(is_runtime_module);
  if !chunk_carries_runtime {
    if let Some(prelude) = dev_graph_prelude.take() {
      source_joiner.append_source(prelude);
    }
  }

  // If there is no concatenate_wrapping_modules, just concate all modules by exec order.
  if ctx.chunk.module_groups.is_empty() {
    module_sources.iter().for_each(
      |RenderedModuleSource { sources: module_render_output, module_idx, .. }| {
        if let Some(emitted_sources) = module_render_output {
          for source in emitted_sources.as_ref() {
            source_joiner.append_source(source);
          }
        }
        if let Some(prelude) = dev_graph_prelude.take_if(|_| is_runtime_module(*module_idx)) {
          source_joiner.append_source(prelude);
        }
      },
    );
    return;
  }
  let module_idx_to_source_idx = module_sources.iter().enumerate().fold(
    FxHashMap::default(),
    |mut acc, (idx, module_source)| {
      acc.insert(module_source.module_idx, idx);
      acc
    },
  );
  let profiler_names = ctx.options.profiler_names;
  let is_pife_for_module_wrappers_enabled =
    ctx.options.optimization.is_pife_for_module_wrappers_enabled();
  for group in &ctx.chunk.module_groups {
    // If the group is not belong to any concatenated module, we just render it as a single module.
    if group.modules.len() == 1 {
      let source =
        module_sources.get(module_idx_to_source_idx[&group.entry]).expect("should have source");
      if let Some(emitted_sources) = source.sources.as_ref() {
        for source in emitted_sources.as_ref() {
          source_joiner.append_source(source);
        }
      }
      if let Some(prelude) = dev_graph_prelude.take_if(|_| is_runtime_module(group.entry)) {
        source_joiner.append_source(prelude);
      }
      continue;
    }
    // Concatenate hoisted functions and comma-join hoisted vars across the group's modules by
    // appending each element directly into the accumulators, instead of allocating a temporary
    // joined `String` per module just to append it.
    let mut hoisted_fns = String::new();
    let mut hoisted_vars = String::new();
    for idx in &group.modules {
      let Some(render_concatenated_module) =
        ctx.chunk.module_idx_to_render_concatenated_module.get(idx)
      else {
        continue;
      };
      for hoisted_fn in &render_concatenated_module.hoisted_functions_or_module_ns_decl {
        hoisted_fns.push_str(hoisted_fn);
      }
      for hoisted_var in &render_concatenated_module.hoisted_vars {
        if !hoisted_vars.is_empty() {
          hoisted_vars.push_str(", ");
        }
        hoisted_vars.push_str(hoisted_var);
      }
    }

    let entry_module_stable_id = ctx.link_output.module_table[group.entry].stable_id();
    // render var init_entry = __esm("", () => {
    let rendered_esm_runtime_expr = ctx.chunk.module_idx_to_render_concatenated_module
      [&group.entry]
      .rendered_esm_runtime_expr
      .as_ref()
      .unwrap()
      .trim_end_matches([';', '\n']);
    let wrap_name = ctx.chunk.module_idx_to_render_concatenated_module[&group.entry]
      .wrap_ref_name
      .as_ref()
      .unwrap();

    // The concatenated closure holds the bodies of every module in the group. If the group's
    // entry is TLA-tainted (has top-level `await`, or awaits a TLA importee's `init` call), those
    // `await`s land inside this closure, so it must be `async`. This mirrors the flag threaded
    // into `new_esm_wrapper_stmt` for the non-concatenated wrapper.
    let is_async = ctx.link_output.metas[group.entry].is_tla_or_contains_tla_dependency;

    if !hoisted_fns.is_empty() {
      source_joiner.append_source(hoisted_fns);
    }
    if !hoisted_vars.is_empty() {
      source_joiner.append_source(concat_string!("var ", hoisted_vars, ";"));
    }

    source_joiner.append_source(concat_string!(
      "var ",
      wrap_name,
      " = ",
      rendered_esm_runtime_expr,
      "(",
      if profiler_names {
        concat_string!("{\"", entry_module_stable_id, "\": ")
      } else {
        String::new()
      },
      if is_pife_for_module_wrappers_enabled { "(" } else { "" },
      if is_async { "async () => {" } else { "() => {" }
    ));
    // we render each module in the group by exec order.
    group.modules.iter().for_each(|module_idx| {
      if let Some(rendered) =
        module_sources.get(module_idx_to_source_idx[module_idx]).and_then(|m| m.sources.as_ref())
      {
        for source in rendered.iter() {
          source_joiner.append_source(source);
        }
      }
    });
    let mut postfix = "}".to_string();
    if is_pife_for_module_wrappers_enabled {
      postfix += ")";
    }
    if profiler_names {
      postfix += "}";
    }
    postfix += ");\n";
    source_joiner.append_source(postfix);
    if let Some(prelude) =
      dev_graph_prelude.take_if(|_| group.modules.iter().copied().any(is_runtime_module))
    {
      source_joiner.append_source(prelude);
    }
  }
  if let Some(prelude) = dev_graph_prelude.take() {
    source_joiner.append_source(prelude);
  }
}

fn render_esm_chunk_imports(ctx: &GenerateContext<'_>) -> Option<String> {
  let mut s = String::new();
  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_id];
    let mut default_alias = vec![];
    // Track seen canonical refs to avoid duplicate imports.
    // Multiple import_refs can resolve to the same canonical_ref (e.g., re-exports from CJS modules),
    // and we only need to import once per unique canonical symbol.
    let mut seen_canonical_refs: FxHashSet<SymbolRef> = FxHashSet::default();
    let mut specifiers = items
      .iter()
      .filter_map(|item| {
        let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(item.import_ref);
        // Skip if we've already processed this canonical symbol
        if !seen_canonical_refs.insert(canonical_ref) {
          return None;
        }
        let imported = ctx
          .link_output
          .symbol_db
          .canonical_name_for_or_original(canonical_ref, &ctx.chunk.canonical_names);
        let alias = &ctx.render_export_items_index_vec[*exporter_id]
          .get(&item.import_ref)
          .expect("should have export item index")[0];
        if alias.as_str() == imported {
          Some(to_module_import_export_name(alias.as_str()))
        } else {
          if alias.as_str() == "default" {
            default_alias.push(imported.into());
            return None;
          }
          Some(concat_string!(to_module_import_export_name(alias), " as ", imported))
        }
      })
      .collect::<Vec<_>>();
    specifiers.sort_unstable();

    s.push_str(&create_import_declaration(
      &ctx.link_output.module_table,
      specifiers,
      &default_alias,
      &ctx.chunk.import_path_for(importee_chunk),
      None,
    ));
  });
  let mut rendered_external_import_namespace_modules = FxHashSet::default();
  // Only an ESM entry module re-exports externals at entry level.
  let renders_entry_level = ctx
    .chunk
    .entry_module(&ctx.link_output.module_table)
    .is_some_and(|module| matches!(module.exports_kind, ExportsKind::Esm));
  let bare_import_attributes = bare_import_attributes(ctx);
  // render external imports
  ctx.chunk.direct_imports_from_external_modules.iter().for_each(|(importee_id, named_imports)| {
    let importee = &ctx.link_output.module_table[*importee_id]
      .as_external()
      .expect("Should be external module here");
    // An entry-level external is upgraded to `export * from` in place. That statement loads the
    // external too, so the external needs no bare import.
    // See internal-docs/external-star-exports/implementation.md.
    let entry_level =
      renders_entry_level.then(|| ctx.chunk.entry_level_external(*importee_id)).flatten();
    let mut has_importee_imported = entry_level.is_some();
    let mut import_attribute = None;
    // TODO: Warning same import record has different import attributes. https://tinyurl.com/2ddnbbc8
    named_imports.iter().for_each(|(idx, named_import)| {
      let module = ctx.link_output.module_table[*idx].as_normal().unwrap();
      if module.import_attribute_map.contains_key(&named_import.record_idx) {
        if import_attribute.is_none() {
          import_attribute = Some((module.idx, named_import.record_idx));
        }
      }
    });
    s += &render_named_imports(
      ctx,
      importee,
      named_imports.iter(),
      &mut has_importee_imported,
      &mut rendered_external_import_namespace_modules,
      import_attribute,
      bare_import_attributes.get(importee_id).copied(),
    );
    if let Some(entry_level) = entry_level {
      let with_clause = entry_level.attribute_record.and_then(|(module_idx, rec_idx)| {
        ctx.link_output.module_table[module_idx].as_normal()?.import_attribute_map.get(&rec_idx)
      });
      s.push_str(&concat_string!(
        "export * from ",
        escape(&importee.get_import_path(ctx.chunk, ctx.resolved_paths)),
        with_clause.map(|attr| concat_string!(" ", attr.to_string())).unwrap_or_default(),
        ";\n"
      ));
    }
  });
  (!s.is_empty()).then_some(s)
}

/// The `with` clause that a bare import of each external keeps: the one of the first static record
/// in this chunk that imports the external with one.
fn bare_import_attributes(
  ctx: &GenerateContext<'_>,
) -> FxHashMap<ModuleIdx, (ModuleIdx, ImportRecordIdx)> {
  let mut ret = FxHashMap::default();
  for module_idx in &ctx.chunk.modules {
    let Some(module) = ctx.link_output.module_table[*module_idx].as_normal() else {
      continue;
    };
    if module.import_attribute_map.is_empty() {
      continue;
    }
    for (rec_idx, rec) in module.import_records.iter_enumerated() {
      if rec.kind != ImportKind::Import || !module.import_attribute_map.contains_key(&rec_idx) {
        continue;
      }
      if let Some(importee_idx) = rec.resolved_module
        && ctx.link_output.module_table[importee_idx].is_external()
      {
        ret.entry(importee_idx).or_insert((module.idx, rec_idx));
      }
    }
  }
  ret
}

fn create_import_declaration(
  module_table: &ModuleTable,
  mut specifiers: Vec<String>,
  default_alias: &[ArcStr],
  path: &str,
  with_clause: Option<(ModuleIdx, ImportRecordIdx)>,
) -> String {
  let mut ret = String::new();
  let with_clause_string = with_clause.and_then(|(module_idx, record_idx)| {
    let module = module_table[module_idx].as_normal()?;
    let import_attribute = module.import_attribute_map.get(&record_idx)?;
    Some(import_attribute.to_string())
  });
  let first_default_alias = match &default_alias {
    [] => None,
    [first] => Some(first),
    [first, rest @ ..] => {
      specifiers.extend(rest.iter().map(|item| concat_string!("default as ", item)));
      Some(first)
    }
  };
  if !specifiers.is_empty() {
    ret.push_str("import ");
    if let Some(first_default_alias) = first_default_alias {
      ret.push_str(first_default_alias);
      ret.push_str(", ");
    }
    ret.push_str("{ ");
    ret.push_str(&specifiers.join(", "));
    ret.push_str(" } from ");
    ret.push_str(&escape(path));
  } else if let Some(first_default_alias) = first_default_alias {
    ret.push_str("import ");
    ret.push_str(first_default_alias);
    ret.push_str(" from ");
    ret.push_str(&escape(path));
  } else {
    ret.push_str("import \"");
    ret.push_str(path);
    ret.push('"');
  }

  if let Some(with_clause) = with_clause_string {
    ret.push(' ');
    ret.push_str(&with_clause);
  }
  ret.push_str(";\n");
  ret
}

fn render_named_imports<'a, I>(
  ctx: &GenerateContext<'_>,
  importee: &ExternalModule,
  named_imports: I,
  is_importee_rendered: &mut bool,
  rendered_external_import_namespace_modules: &mut FxHashSet<ModuleIdx>,
  with_clause: Option<(ModuleIdx, ImportRecordIdx)>,
  bare_import_with_clause: Option<(ModuleIdx, ImportRecordIdx)>,
) -> String
where
  I: Iterator<Item = &'a (ModuleIdx, rolldown_common::NamedImport)>,
{
  let mut s = String::new();
  let mut default_alias = vec![];
  let specifiers = named_imports
    .filter_map(|(_importer, named_import)| {
      let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(named_import.imported_as);
      // A named import that is itself re-exported skips the external binding merger
      // (`bind_imports_and_exports`), so its canonical ref stays importer-local; its usage
      // is still answered by `used_symbol_refs` until exports get their own tracking.
      let is_used = if ctx.link_output.module_table[canonical_ref.owner].is_external() {
        ctx.link_output.used_external_symbols.contains(&canonical_ref)
      } else {
        ctx.used_symbol_refs.contains(&canonical_ref)
      };
      if !is_used {
        return None;
      }
      let alias = ctx
        .link_output
        .symbol_db
        .canonical_name_for_or_original(canonical_ref, &ctx.chunk.canonical_names);
      match &named_import.imported {
        Specifier::Star => {
          if rendered_external_import_namespace_modules.contains(&importee.idx) {
            return None;
          }
          rendered_external_import_namespace_modules.insert(importee.idx);
          *is_importee_rendered = true;
          s.push_str("import * as ");
          s.push_str(alias);
          s.push_str(" from ");
          s.push_str(&escape(&importee.get_import_path(ctx.chunk, ctx.resolved_paths)));
          s.push_str(";\n");
          None
        }
        Specifier::Literal(imported) => {
          if alias == imported.as_str() {
            Some(alias.into())
          } else {
            if imported.as_str() == "default" {
              default_alias.push(alias.into());
              return None;
            }
            let imported = to_module_import_export_name(imported);
            Some(concat_string!(imported, " as ", alias))
          }
        }
      }
    })
    .sorted_unstable()
    .dedup()
    .collect::<Vec<_>>();
  default_alias.sort_unstable();
  default_alias.dedup();

  if !specifiers.is_empty()
    || !default_alias.is_empty()
    || (importee.side_effects.has_side_effects() && !*is_importee_rendered)
  {
    *is_importee_rendered = true;
    let with_clause = if specifiers.is_empty() && default_alias.is_empty() {
      with_clause.or(bare_import_with_clause)
    } else {
      with_clause
    };
    s.push_str(&create_import_declaration(
      &ctx.link_output.module_table,
      specifiers,
      &default_alias,
      &importee.get_import_path(ctx.chunk, ctx.resolved_paths),
      with_clause,
    ));
  }
  s
}
