use itertools::Itertools;
use rolldown_common::{
  AddonRenderContext, ChunkIdx, ConcatenateWrappedModuleKind, ConcatenatedGroupEvent, ExportsKind,
  ExternalModule, ImportKind, ImportRecordMeta, ModuleIdx, NamedImport, RUNTIME_MODULE_KEY,
  Specifier, SymbolRef, WrapKind,
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
  utils::{
    chunk::render_chunk_exports::{render_chunk_exports, render_wrapped_entry_chunk},
    render_ecma_module::module_debug_comment,
  },
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
          // With `experimental.importAttributes` the attributes are part of the external module,
          // so every import and re-export of it carries them. Without it there are none.
          let with_clause = m.attributes.to_with_clause(ctx.options.charset.is_ascii());
          source_joiner.append_source(concat_string!(
            "export * from ",
            to_ecma_string_literal(&ext_name, ctx.options.charset.is_ascii()),
            with_clause.map(|attr| concat_string!(" ", attr)).unwrap_or_default(),
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
  // `group_deferred_modules` makes no groups in dev mode, so no prelude is pending here.
  debug_assert!(dev_graph_prelude.is_none());
  render_deferred_groups(ctx, module_sources, source_joiner);
}

/// Renders a chunk with deferred groups (`group_deferred_modules`):
/// - A member adds, at its own place, only its declarations.
/// - A root adds its declarations and the start of its wrapper, then the CommonJS wrappers of
///   the group, the bodies of the members in the order of evaluation, its own body, and the end
///   of its wrapper.
///
/// See internal-docs/import-defer/implementation.md ("Deferred groups").
fn render_deferred_groups<'code>(
  ctx: &GenerateContext<'_>,
  module_sources: &'code [RenderedModuleSource],
  source_joiner: &mut SourceJoiner<'code>,
) {
  let source_by_module =
    module_sources.iter().map(|source| (source.module_idx, source)).collect::<FxHashMap<_, _>>();
  // The CommonJS modules of a group keep their `require_*` wrapper, and the whole wrapper
  // declaration goes in the wrapper of the root (`commonjs_modules_of_groups`).
  let mut commonjs_by_root: FxHashMap<ModuleIdx, Vec<ModuleIdx>> = FxHashMap::default();
  for group in &ctx.chunk.module_groups {
    let commonjs = group
      .modules
      .iter()
      .copied()
      .filter(|&idx| matches!(ctx.link_output.metas[idx].wrap_kind(), WrapKind::Cjs))
      .collect_vec();
    if !commonjs.is_empty() {
      commonjs_by_root.insert(group.entry, commonjs);
    }
  }
  let moved_commonjs = commonjs_by_root.values().flatten().copied().collect::<FxHashSet<_>>();
  for module_source in module_sources {
    if moved_commonjs.contains(&module_source.module_idx) {
      continue;
    }
    for source in module_source.sources_of_group_declarations() {
      source_joiner.append_source(source);
    }
    if !matches!(
      ctx.link_output.metas[module_source.module_idx].concatenated_wrapped_module_kind,
      ConcatenateWrappedModuleKind::Root
    ) {
      continue;
    }
    let Some((wrapper_start, wrapper_end)) = ctx
      .chunk
      .module_idx_to_render_concatenated_module
      .get(&module_source.module_idx)
      .and_then(|parts| parts.root_wrapper.as_ref())
    else {
      debug_assert!(false, "the finalizer gives each root the text of its wrapper");
      continue;
    };
    // The debug comment of a module comes with its first source. A root with no declarations
    // starts at its wrapper.
    if module_source.sources_of_group_declarations().is_empty()
      && let Some(comment) = group_debug_comment(ctx, module_source.module_idx)
    {
      source_joiner.append_source(comment);
    }
    source_joiner.append_source(wrapper_start.clone());
    // The wrapper declarations come first: the member bodies and the pre-calls can call them.
    let mut after_members = false;
    for commonjs in commonjs_by_root.get(&module_source.module_idx).into_iter().flatten() {
      if let Some(commonjs_source) = source_by_module.get(commonjs) {
        for source in commonjs_source.all_sources() {
          source_joiner.append_source(source);
          after_members = true;
        }
      }
    }
    let (members_appended, root_steps) =
      append_group_member_bodies(ctx, module_source.module_idx, &source_by_module, source_joiner);
    after_members |= members_appended;
    let body = module_source.sources_of_group_body();
    if after_members
      && (!root_steps.is_empty() || !body.is_empty())
      && let Some(comment) = group_debug_comment(ctx, module_source.module_idx)
    {
      source_joiner.append_source(comment);
    }
    if !root_steps.is_empty() {
      source_joiner.append_source(root_steps.join("\n"));
    }
    for source in body {
      source_joiner.append_source(source);
    }
    source_joiner.append_source(wrapper_end.clone());
  }
}

/// The debug comment of the part of a module that is in the wrapper of a root.
fn group_debug_comment(ctx: &GenerateContext<'_>, module_idx: ModuleIdx) -> Option<String> {
  ctx.link_output.module_table[module_idx]
    .as_normal()
    .and_then(|module| module_debug_comment(module, ctx.options))
}

/// Adds the body of each member that the group of `root` evaluates, depth first in the order of
/// the steps (`ConcatenatedGroupEvent`), as the spec `InnerModuleEvaluation` does: a member body
/// comes after the bodies of the members that it imports. Before a member body, it adds the
/// pending steps of the importer, in their order:
/// - the calls of the functions outside the group that the importer calls before that member
///   (the pre-calls), so that those modules still evaluate first. The importer calls them again
///   in its body, and the second call does nothing. A pre-call that an earlier pre-call of the
///   same group already made is not added again.
/// - the statements that the finalizer moved out of the body of the importer (`Init`), so that
///   the member body can read their bindings.
///
/// A moved statement with no member body after it comes before the body of its module, after
/// the pending pre-calls before it. Returns true if it added a source, and the steps that come
/// before the body of the root.
fn append_group_member_bodies<'code, 'ctx>(
  ctx: &'ctx GenerateContext<'_>,
  root: ModuleIdx,
  source_by_module: &FxHashMap<ModuleIdx, &'code RenderedModuleSource>,
  source_joiner: &mut SourceJoiner<'code>,
) -> (bool, Vec<&'ctx str>) {
  let mut appended = false;
  let events_of = |module_idx: ModuleIdx| {
    ctx
      .chunk
      .module_idx_to_render_concatenated_module
      .get(&module_idx)
      .map_or(&[][..], |parts| parts.group_events.as_slice())
  };
  let mut visited = FxHashSet::from_iter([root]);
  let mut pre_called: FxHashSet<&str> = FxHashSet::default();
  let mut root_steps = vec![];
  // (module, index of the next step, the pending steps)
  let mut stack: Vec<(ModuleIdx, usize, Vec<&ConcatenatedGroupEvent>)> = vec![(root, 0, vec![])];
  while let Some(top) = stack.last_mut() {
    let events = events_of(top.0);
    let Some(event) = events.get(top.1) else {
      let (module_idx, _, pending) = stack.pop().expect("the stack is not empty");
      let steps = group_steps_to_write(&pending, &mut pre_called, true);
      if module_idx == root {
        root_steps = steps;
        continue;
      }
      let body = source_by_module
        .get(&module_idx)
        .map_or(&[][..], |member_source| member_source.sources_of_group_body());
      if !body.is_empty() || !steps.is_empty() {
        if let Some(comment) = group_debug_comment(ctx, module_idx) {
          source_joiner.append_source(comment);
        }
        appended = true;
      }
      if !steps.is_empty() {
        source_joiner.append_source(steps.join("\n"));
      }
      for source in body {
        source_joiner.append_source(source);
      }
      continue;
    };
    top.1 += 1;
    match event {
      ConcatenatedGroupEvent::Call(_) | ConcatenatedGroupEvent::Init(_) => top.2.push(event),
      ConcatenatedGroupEvent::Member(member) => {
        if !visited.insert(*member) {
          continue;
        }
        let steps = group_steps_to_write(&top.2, &mut pre_called, false);
        top.2.clear();
        if !steps.is_empty() {
          source_joiner.append_source(steps.join("\n"));
          appended = true;
        }
        stack.push((*member, 0, vec![]));
      }
    }
  }
  (appended, root_steps)
}

/// The texts of the pending steps of a module to write now, in their order. A pre-call that the
/// group already made is not written again. Each moved statement is written, because each one
/// is pending only one time. With `until_last_init`, the steps end at the last moved statement:
/// the body of the module makes the calls after it.
fn group_steps_to_write<'ctx>(
  pending: &[&'ctx ConcatenatedGroupEvent],
  pre_called: &mut FxHashSet<&'ctx str>,
  until_last_init: bool,
) -> Vec<&'ctx str> {
  let end = if until_last_init {
    pending
      .iter()
      .rposition(|event| matches!(event, ConcatenatedGroupEvent::Init(_)))
      .map_or(0, |idx| idx + 1)
  } else {
    pending.len()
  };
  pending[..end]
    .iter()
    .filter_map(|event| match event {
      ConcatenatedGroupEvent::Call(call) => pre_called.insert(call).then_some(call.as_str()),
      ConcatenatedGroupEvent::Init(init) => Some(init.as_str()),
      ConcatenatedGroupEvent::Member(_) => None,
    })
    .collect()
}

fn render_esm_chunk_imports(ctx: &GenerateContext<'_>) -> Option<String> {
  let mut s = String::new();
  let ascii_only = ctx.options.charset.is_ascii();
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
        let rendered_imported = escape_non_ascii(imported, ascii_only);
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

    s.push_str(&create_import_declaration(
      specifiers,
      &default_alias,
      &ctx.chunk.import_path_for(importee_chunk),
      None,
      ascii_only,
    ));
  });
  let mut rendered_external_import_namespace_modules = FxHashSet::default();
  // render external imports
  ctx.chunk.direct_imports_from_external_modules.iter().for_each(|(importee_id, named_imports)| {
    let importee = &ctx.link_output.module_table[*importee_id]
      .as_external()
      .expect("Should be external module here");
    let mut has_importee_imported = false;
    s += &render_named_imports(
      ctx,
      importee,
      named_imports.iter(),
      &mut has_importee_imported,
      &mut rendered_external_import_namespace_modules,
      has_evaluation_phase_import(ctx, *importee_id),
    );
  });
  (!s.is_empty()).then_some(s)
}

/// Whether `named_import` comes from `import defer * as ns from '...'`.
fn is_defer_phase_import(
  ctx: &GenerateContext<'_>,
  importer: ModuleIdx,
  named_import: &NamedImport,
) -> bool {
  ctx.link_output.module_table[importer]
    .as_normal()
    .is_some_and(|module| module.import_records[named_import.record_idx].is_defer_phase())
}

/// Whether a module of this chunk imports the external in the evaluation phase. `import defer`
/// does not evaluate the external when the chunk loads. Thus, an external that this chunk imports
/// only in the defer phase (`experimental.importDeferExternal: 'preserve'`) gets no bare
/// `import 'ext'` for its side effects.
fn has_evaluation_phase_import(ctx: &GenerateContext<'_>, external_idx: ModuleIdx) -> bool {
  !ctx.options.experimental.is_import_defer_enabled()
    || ctx.chunk.modules.iter().any(|module_idx| {
      ctx.link_output.module_table[*module_idx].as_normal().is_some_and(|module| {
        module.import_records.iter().any(|rec| {
          rec.kind == ImportKind::Import
            && rec.resolved_module == Some(external_idx)
            && !rec.is_defer_phase()
            && !rec.meta.contains(ImportRecordMeta::IsExportStar)
        })
      })
    })
}

fn create_import_declaration(
  mut specifiers: Vec<String>,
  default_alias: &[String],
  path: &str,
  with_clause: Option<String>,
  ascii_only: bool,
) -> String {
  let mut ret = String::new();
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

  if let Some(with_clause) = with_clause {
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
  has_evaluation_phase_import: bool,
) -> String
where
  I: Iterator<Item = &'a (ModuleIdx, rolldown_common::NamedImport)>,
{
  let mut s = String::new();
  let ascii_only = ctx.options.charset.is_ascii();
  let mut default_alias = vec![];
  let mut rendered_deferred_aliases = FxHashSet::default();
  let specifiers = named_imports
    .filter_map(|(importer, named_import)| {
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
      if is_defer_phase_import(ctx, *importer, named_import) {
        // `import defer * as ns from 'ext'` (kept with `experimental.importDeferExternal:
        // 'preserve'`). It is a separate declaration, and it does not evaluate the external.
        if rendered_deferred_aliases.insert(rendered_alias.to_string()) {
          s.push_str("import defer * as ");
          s.push_str(&rendered_alias);
          s.push_str(" from ");
          s.push_str(&to_ecma_string_literal(
            &importee.get_import_path(ctx.chunk, ctx.resolved_paths),
            ascii_only,
          ));
          if let Some(with_clause) = importee.attributes.to_with_clause(ascii_only) {
            s.push(' ');
            s.push_str(&with_clause);
          }
          s.push_str(";\n");
        }
        return None;
      }
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
          if let Some(with_clause) = importee.attributes.to_with_clause(ascii_only) {
            s.push(' ');
            s.push_str(&with_clause);
          }
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
    || (importee.side_effects.has_side_effects()
      && !*is_importee_rendered
      && has_evaluation_phase_import)
  {
    *is_importee_rendered = true;
    s.push_str(&create_import_declaration(
      specifiers,
      &default_alias,
      &importee.get_import_path(ctx.chunk, ctx.resolved_paths),
      importee.attributes.to_with_clause(ascii_only),
      ascii_only,
    ));
  }
  s
}
