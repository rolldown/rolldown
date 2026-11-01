use std::collections::VecDeque;

use itertools::Itertools;
use rolldown_common::{
  AddonRenderContext, ChunkIdx, ExportsKind, ExternalModule, ImportAttribute, ImportKind,
  ImportRecordIdx, ImportRecordMeta, Module, ModuleIdx, ModuleTable, RUNTIME_MODULE_KEY, Specifier,
  SymbolRef,
};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{
  concat_string,
  ecmascript::{escape_non_ascii, to_ecma_string_literal, to_module_import_export_name_with_ascii},
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  share_factory::render_inline_records,
  utils::{is_use_strict_directive, render_chunk_directives},
};
use crate::{
  ecmascript::ecma_generator::{RenderedModuleSource, RenderedModuleSources},
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::{render_chunk_exports, render_wrapped_entry_chunk},
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

  if let Some(entry_module) = ctx.chunk.entry_module(&ctx.link_output.module_table) {
    if matches!(entry_module.exports_kind, ExportsKind::Esm) {
      for importee_idx in ctx.chunk.entry_level_external_module_idx.iter().copied() {
        let importee = &ctx.link_output.module_table[importee_idx];
        if let Some(m) = importee.as_external() {
          let ext_name = m.get_import_path(ctx.chunk, ctx.resolved_paths);
          // Preserve the `with { ... }` import attribute from the originating
          // `export * from "..." with { ... }` record (issue #9160) instead of dropping it.
          // Follow this entry's export-star chain to the record that resolved to this external and
          // reuse its attribute. This remains entry-specific when several facades share one
          // implementation chunk. Notes:
          // - If no such record carries an attribute, `with_clause` is `None` and we emit the
          //   plain `export * from` exactly as before (safe degradation, no regression). This is
          //   also what happens if the originating record is unavailable.
          // - If several records re-export the same external with *different* attributes (a
          //   pathological, conflicting input), the first attributed record in the entry's
          //   breadth-first export-star traversal wins.
          let with_clause =
            find_entry_level_external_import_attribute(ctx, entry_module.idx, importee_idx);
          // An absent attribute is just an empty suffix, so both cases collapse to a
          // single `append_source` (same shape `create_import_declaration` uses below).
          source_joiner.append_source(concat_string!(
            "export * from ",
            to_ecma_string_literal(&ext_name, ctx.options.charset.is_ascii()),
            with_clause.map(|attr| concat_string!(" ", attr.to_string())).unwrap_or_default(),
            "\n"
          ));
        }
      }
    }
  }

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

fn find_entry_level_external_import_attribute<'a>(
  ctx: &'a GenerateContext<'_>,
  entry_module_idx: ModuleIdx,
  external_module_idx: ModuleIdx,
) -> Option<&'a ImportAttribute> {
  let module_table = &ctx.link_output.module_table;
  // Order-wrap entry facades render entry-level re-exports while the owning record lives in
  // another chunk, so strict follows the export-star chain from the entry. Flag-off keeps
  // main's same-chunk scan so its output stays byte-identical.
  if !ctx.options.is_strict_execution_order_enabled() {
    return ctx.chunk.modules.iter().find_map(|module_idx| {
      let module = module_table[*module_idx].as_normal()?;
      module.import_records.iter_enumerated().find_map(|(rec_idx, rec)| {
        (rec.resolved_module == Some(external_module_idx)
          && rec.meta.contains(ImportRecordMeta::IsExportStar)
          && rec.meta.contains(ImportRecordMeta::EntryLevelExternal))
        .then(|| module.import_attribute_map.get(&rec_idx))
        .flatten()
      })
    });
  }

  let mut queue = VecDeque::from([entry_module_idx]);
  let mut visited = FxHashSet::default();

  while let Some(module_idx) = queue.pop_front() {
    if !visited.insert(module_idx) {
      continue;
    }
    let Some(module) = module_table[module_idx].as_normal() else {
      continue;
    };

    for (rec_idx, rec) in module.import_records.iter_enumerated() {
      if !rec.meta.contains(ImportRecordMeta::IsExportStar) {
        continue;
      }
      let Some(importee_idx) = rec.resolved_module else {
        continue;
      };
      if importee_idx == external_module_idx
        && rec.meta.contains(ImportRecordMeta::EntryLevelExternal)
      {
        if let Some(import_attribute) = module.import_attribute_map.get(&rec_idx) {
          return Some(import_attribute);
        }
        continue;
      }
      if module_table[importee_idx].is_normal() {
        queue.push_back(importee_idx);
      }
    }
  }

  None
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
      crate::utils::native_records::dev_graph_rows(
        ctx.chunk_idx,
        ctx.chunk,
        &ctx.link_output.module_table,
        ctx.chunk_graph,
        &ctx.link_output.metas,
        crate::utils::native_records::enabled(ctx.options),
      ),
      &ctx.link_output.user_defined_entry_modules,
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
  let mut internal_imports = vec![];
  let ascii_only = ctx.options.charset.is_ascii();
  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let mut s = String::new();
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_id];
    let mut default_alias = vec![];
    let mut native_namespaces = vec![];
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
        let rendered_imported = escape_non_ascii(imported, ascii_only);
        if crate::utils::native_records::is_native_namespace(
          ctx.options,
          ctx.link_output,
          canonical_ref,
        ) && importee_chunk
          .entry_module_idx()
          .is_some_and(|module| module == canonical_ref.owner)
        {
          native_namespaces.push(rendered_imported.into_owned());
          return None;
        }
        let alias = &ctx.render_export_items_index_vec[*exporter_id]
          .get(&item.import_ref)
          .expect("should have export item index")[0];
        if alias.as_str() == imported {
          Some(to_module_import_export_name_with_ascii(alias.as_str(), ascii_only))
        } else {
          if alias.as_str() == "default" {
            default_alias.push(rendered_imported.into_owned());
            return None;
          }
          Some(concat_string!(
            to_module_import_export_name_with_ascii(alias, ascii_only),
            " as ",
            rendered_imported
          ))
        }
      })
      .collect::<Vec<_>>();
    specifiers.sort_unstable();

    let path = ctx.chunk.import_path_for(importee_chunk);
    native_namespaces.sort_unstable();
    for namespace in &native_namespaces {
      s.push_str(&concat_string!(
        "import * as ",
        namespace,
        " from ",
        to_ecma_string_literal(&path, ascii_only),
        ";\n"
      ));
    }
    if native_namespaces.is_empty() || !specifiers.is_empty() || !default_alias.is_empty() {
      s.push_str(&create_import_declaration(
        &ctx.link_output.module_table,
        specifiers,
        &default_alias,
        &path,
        None,
        ascii_only,
      ));
    }
    internal_imports.push((*exporter_id, s));
  });
  let has_deferred_imports =
    ctx.chunk.modules.iter().any(|idx| !ctx.link_output.metas[*idx].deferred_imports.is_empty());
  if has_deferred_imports && crate::utils::native_records::enabled(ctx.options) {
    let mut imports: FxHashMap<_, _> = internal_imports.into_iter().collect();
    // The runtime is never an authored dependency and must precede registration glue.
    if let Some(runtime) = ctx.chunk_graph.module_to_chunk[ctx.link_output.runtime.id()]
      && let Some(source) = imports.remove(&runtime)
    {
      s.push_str(&source);
    }
    let mut deferred_only = FxHashSet::default();
    for idx in &ctx.chunk.modules {
      let Some(module) = ctx.link_output.module_table[*idx].as_normal() else {
        continue;
      };
      let meta = &ctx.link_output.metas[*idx];
      for (record_idx, record) in module.import_records.iter_enumerated() {
        if !record.kind.is_static() {
          continue;
        }
        let Some(target) = record.resolved_module else {
          continue;
        };
        let target_chunk = ctx.chunk_graph.module_to_chunk[target];
        if let Some(binding) = meta.deferred_imports.get(&record_idx) {
          let name = ctx
            .link_output
            .symbol_db
            .canonical_name_for_or_original(*binding, &ctx.chunk.canonical_names);
          let path = match &ctx.link_output.module_table[target] {
            Module::Normal(importee) => {
              let Some(chunk) = target_chunk else { continue };
              deferred_only.insert(chunk);
              let target = &ctx.chunk_graph.chunk_table[chunk];
              if crate::utils::import_options::is_json_module(importee) {
                crate::utils::native_records::deferred_json_import_path(ctx.chunk, target)
              } else {
                ctx.chunk.import_path_for(target)
              }
            }
            Module::External(module) => {
              module.get_import_path(ctx.chunk, ctx.resolved_paths).to_string()
            }
          };
          s.push_str(&concat_string!(
            "import defer * as ",
            escape_non_ascii(name, ascii_only),
            " from ",
            to_ecma_string_literal(&path, ascii_only)
          ));
          if ctx.link_output.module_table[target].is_external()
            && let Some(attribute) = module.import_attribute_map.get(&record_idx)
          {
            s.push(' ');
            s.push_str(&attribute.to_string());
          }
          s.push_str(";\n");
        } else if let Some(chunk) = target_chunk
          && let Some(source) = imports.remove(&chunk)
        {
          // Eager and deferred requests for the same module are separate edges:
          // an earlier deferred request must not advance the eager request's order.
          s.push_str(&source);
        }
      }
    }
    for (chunk, items) in &ctx.chunk.imports_from_other_chunks {
      if let Some(source) = imports.remove(chunk)
        && (!deferred_only.contains(chunk) || !items.is_empty())
      {
        s.push_str(&source);
      }
    }
  } else {
    for (_, source) in internal_imports {
      s.push_str(&source);
    }
  }
  let mut rendered_external_import_namespace_modules = FxHashSet::default();
  let bare_import_attributes = bare_import_attributes(ctx);
  // render external imports
  ctx.chunk.direct_imports_from_external_modules.iter().for_each(|(importee_id, named_imports)| {
    let importee = &ctx.link_output.module_table[*importee_id]
      .as_external()
      .expect("Should be external module here");
    let mut has_importee_imported = false;
    let mut import_attribute = None;
    let named_imports: Vec<_> = named_imports
      .iter()
      .filter(|(idx, import)| {
        !ctx.link_output.metas[*idx].deferred_imports.contains_key(&import.record_idx)
      })
      .collect();
    let has_deferred_edge = ctx.chunk.modules.iter().any(|idx| {
      let Some(module) = ctx.link_output.module_table[*idx].as_normal() else { return false };
      ctx.link_output.metas[*idx].deferred_imports.keys()
        .any(|record| module.import_records[*record].resolved_module == Some(*importee_id))
    });
    let has_eager_edge = ctx.chunk.modules.iter().any(|idx| {
      let Some(module) = ctx.link_output.module_table[*idx].as_normal() else { return false };
      module.import_records.iter_enumerated().any(|(record, import)| {
        import.kind.is_static() && import.resolved_module == Some(*importee_id)
          && !ctx.link_output.metas[*idx].deferred_imports.contains_key(&record)
      })
    });
    if named_imports.is_empty() && has_deferred_edge && !has_eager_edge {
      return;
    }
    // TODO: Warning same import record has different import attributes. https://tinyurl.com/2ddnbbc8
    named_imports.iter().copied().for_each(|(idx, named_import)| {
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
      named_imports.into_iter(),
      &mut has_importee_imported,
      &mut rendered_external_import_namespace_modules,
      import_attribute,
      bare_import_attributes.get(importee_id).copied(),
    );
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
  default_alias: &[String],
  path: &str,
  with_clause: Option<(ModuleIdx, ImportRecordIdx)>,
  ascii_only: bool,
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
    ret.push_str(&to_ecma_string_literal(path, ascii_only));
  } else if let Some(first_default_alias) = first_default_alias {
    ret.push_str("import ");
    ret.push_str(first_default_alias);
    ret.push_str(" from ");
    ret.push_str(&to_ecma_string_literal(path, ascii_only));
  } else {
    ret.push_str("import ");
    ret.push_str(&to_ecma_string_literal(path, ascii_only));
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
  let ascii_only = ctx.options.charset.is_ascii();
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
      let rendered_alias = escape_non_ascii(alias, ascii_only);
      match &named_import.imported {
        Specifier::Star => {
          if rendered_external_import_namespace_modules.contains(&importee.idx) {
            return None;
          }
          rendered_external_import_namespace_modules.insert(importee.idx);
          *is_importee_rendered = true;
          s.push_str("import * as ");
          s.push_str(&rendered_alias);
          s.push_str(" from ");
          s.push_str(&to_ecma_string_literal(
            &importee.get_import_path(ctx.chunk, ctx.resolved_paths),
            ascii_only,
          ));
          s.push_str(";\n");
          None
        }
        Specifier::Literal(imported) => {
          if alias == imported.as_str() {
            Some(rendered_alias.into_owned())
          } else {
            if imported.as_str() == "default" {
              default_alias.push(rendered_alias.into_owned());
              return None;
            }
            let imported = to_module_import_export_name_with_ascii(imported, ascii_only);
            Some(concat_string!(imported, " as ", rendered_alias))
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
      ascii_only,
    ));
  }
  s
}
