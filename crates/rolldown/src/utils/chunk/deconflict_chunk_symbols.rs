use oxc_str::CompactStr;

use crate::{
  stages::{generate_stage::order_wrap_state::OrderWrapState, link_stage::LinkStageOutput},
  utils::{
    external_import_interop::{external_import_needs_interop, specifier_needs_interop},
    renamer::{NestedScopeRenamer, Renamer},
  },
};
use arcstr::ArcStr;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, GetLocalDb, NormalModule, OutputFormat, SymbolRef, WrapKind,
};
use rolldown_utils::ecmascript::legitimize_identifier_name;
use rustc_hash::{FxHashMap, FxHashSet};

#[tracing::instrument(level = "trace", skip_all)]
pub fn deconflict_chunk_symbols(
  chunk_idx: ChunkIdx,
  chunk: &mut Chunk,
  link_output: &LinkStageOutput,
  order_wrap_state: &OrderWrapState,
  order_live_symbols: &FxHashSet<SymbolRef>,
  format: OutputFormat,
  index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
) {
  let mut renamer = Renamer::new(chunk.entry_module_idx(), &link_output.symbol_db, format);
  // Reserve global scope symbols (unresolved references) to prevent generating conflicting names.
  // These are identifiers referenced but not defined in the module's scope (e.g., `console`, `window`).
  chunk
    .modules
    .iter()
    .copied()
    .filter_map(|idx| {
      Some(
        link_output.symbol_db[idx]
          .as_ref()?
          .ast_scopes
          .scoping()
          .root_unresolved_references()
          .keys(),
      )
    })
    .flatten()
    .for_each(|name| {
      renamer.reserve(CompactStr::new(name));
    });

  if matches!(format, OutputFormat::Iife | OutputFormat::Umd | OutputFormat::Cjs) {
    // deconflict iife introduce symbols by external
    // Also AMD, but we don't support them yet.
    chunk
      .direct_imports_from_external_modules
      .iter()
      .map(|(idx, _)| *idx)
      .chain(chunk.entry_level_external_module_idx.iter().copied())
      .filter_map(|idx| link_output.module_table[idx].as_external())
      .for_each(|external_module| {
        renamer.add_symbol_in_root_scope(external_module.namespace_ref, true);
      });

    chunk
      .import_symbol_from_external_modules
      .iter()
      .filter_map(|idx| link_output.module_table[*idx].as_external())
      .for_each(|external_module| {
        renamer.add_symbol_in_root_scope(external_module.namespace_ref, true);
      });
  }

  match chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      let meta = &link_output.metas[module];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(
        |(symbol_ref, came_from_cjs)| {
          if !came_from_cjs {
            renamer.add_symbol_in_root_scope(*symbol_ref, true);
          }
        },
      );
    }
    ChunkKind::Common => {}
  }
  if matches!(format, OutputFormat::Esm) {
    chunk.direct_imports_from_external_modules.iter().for_each(|(module, _)| {
      let db = link_output.symbol_db.local_db(*module);
      db.classic_data.iter_enumerated().for_each(|(symbol, _)| {
        let symbol_ref = (*module, symbol).into();
        if link_output.used_external_symbols.contains(&symbol_ref) {
          renamer.add_symbol_in_root_scope(symbol_ref, true);
        }
      });
    });
  }

  let chunk_scope_captured_names = collect_chunk_scope_captured_names(
    chunk_idx,
    chunk,
    link_output,
    order_wrap_state,
    order_live_symbols,
    format,
    &renamer,
  );

  // The renamer relies on `chunk.modules` being in ascending exec_order so that
  // `.rev()` yields entry-first / descending exec_order — the same priority as
  // `deconflict_order_key`. Enforce that invariant in debug builds (was only a
  // prose + pinned-SHA comment before).
  debug_assert!(
    chunk
      .modules
      .iter()
      .filter_map(|idx| link_output.module_table[*idx].as_normal().map(|m| m.exec_order))
      .is_sorted(),
    "chunk.modules must be in ascending exec_order for deconfliction"
  );

  chunk
    .modules
    .iter()
    .copied()
    // Starts with entry module
    .rev()
    .filter_map(|id| link_output.module_table[id].as_normal())
    .for_each(|module| {
      if let Some(hmr_hot_ref) = module.hmr_hot_ref {
        renamer.add_symbol_in_root_scope(hmr_hot_ref, true);
      }
      // Skip deconflicting top-level symbols for CJS modules since they are wrapped in a function scope
      // and their symbols don't pollute the chunk's root scope.
      let meta = &link_output.metas[module.idx];
      let is_cjs_wrapped_module = matches!(meta.wrap_kind(), WrapKind::Cjs);

      link_output.stmt_infos[module.idx]
        .iter_enumerated()
        // A runtime statement tree-shaking excluded but order wrapping force-includes is rendered
        // and symbol-assigned, so it must reach the renamer too. Mirror the overlay-aware inclusion
        // test the other two consumers already use (`compute_cross_chunk_links` and the module
        // finalizer's `remove_unused_top_level_stmt`); without it a user top-level binding named
        // `__esmMin`/`__esm` co-hosted with the runtime collides with the forced helper declaration.
        .filter(|(idx, stmt_info)| {
          meta.stmt_info_included.has_bit(*idx)
            || order_wrap_state.forces_runtime_stmt(&link_output.runtime, module.idx, stmt_info)
        })
        .for_each(|(_, stmt_info)| {
          for declared_symbol in stmt_info.declared_symbols.iter().filter(|item| item.is_normal()) {
            let symbol_ref = declared_symbol.inner();
            let canonical_ref = link_output.symbol_db.canonical_ref_for(symbol_ref);
            // Import statement declared some symbols that come from other module, those symbol should be skipped
            if canonical_ref.owner != module.idx {
              continue;
            }
            // For CJS wrapped modules, only facade symbols need deconflicting.
            // Facade symbols are synthetic symbols created during linking (e.g., `require_foo` wrapper,
            // namespace objects) that don't exist in the original AST. These are rendered at the chunk's
            // root scope and must be deconflicted. Non-facade (real AST) symbols in CJS modules are
            // wrapped inside the `__commonJS` closure and don't pollute the chunk's root scope.
            let needs_deconflict = if is_cjs_wrapped_module {
              // Note:
              // 1. Some facade symbols may originate from external modules (e.g., namespace objects for external imports).
              // 2. Since we merge external module symbols, external symbol declared in a cjs module also needs to be deconflicted
              let is_facade_or_external = link_output.symbol_db.is_facade_symbol(canonical_ref)
                || stmt_info.import_records.iter().any(|import_rec_idx| {
                  module.import_records[*import_rec_idx]
                    .resolved_module
                    .is_some_and(|module_idx| link_output.module_table[module_idx].is_external())
                });
              // Deconflict bindings that would shadow a name captured by the enclosing
              // `__commonJS` closure (issues #9055, #9375).
              let shadows_chunk_scope_name =
                chunk_scope_captured_names.contains(canonical_ref.name(&link_output.symbol_db));
              is_facade_or_external || shadows_chunk_scope_name
            } else {
              true
            };
            renamer.add_symbol_in_root_scope(symbol_ref, needs_deconflict);
          }
        });
    });

  for synthetic in order_wrap_state.synthetic_statements_for_chunk(chunk_idx) {
    for declared_symbol in synthetic.declared_symbols.iter().filter(|item| item.is_normal()) {
      debug_assert_eq!(declared_symbol.inner().owner, synthetic.owner);
      renamer.add_symbol_in_root_scope(declared_symbol.inner(), true);
    }
  }

  // Though, those symbols in `imports_from_other_chunks` doesn't belong to this chunk, but in the final output, they still behave
  // like declared in this chunk. This is because we need to generate import statements in this chunk to import symbols from other
  // statements. Those `import {...} from './other-chunk.js'` will declared these outside symbols in this chunk, so symbols that
  // point to them can be resolved in runtime.
  // So we add them in the deconflict process to generate conflict-less names in this chunk.
  chunk.imports_from_other_chunks.iter().flat_map(|(_, items)| items.iter()).for_each(|item| {
    renamer.add_symbol_in_root_scope(item.import_ref, true);
  });

  chunk.require_binding_names_for_other_chunks = chunk
    .imports_from_other_chunks
    .iter()
    .map(|(id, _)| {
      (
        *id,
        renamer
          .create_conflictless_name(&legitimize_identifier_name(&format!(
            "require_{}",
            index_chunk_id_to_name[id]
          )))
          .to_string(),
      )
    })
    .collect();

  // Detect mixed-mode external imports: both ESM (node-mode) and non-ESM importers
  // needing interop on the same external. Create a separate binding name for node-mode.
  if matches!(format, OutputFormat::Iife | OutputFormat::Umd | OutputFormat::Cjs) {
    let mut node_mode_names = FxHashMap::default();
    for (ext_idx, named_imports) in &chunk.direct_imports_from_external_modules {
      if !external_import_needs_interop(named_imports) {
        continue;
      }
      let mut has_node_mode = false;
      let mut has_non_node_mode = false;
      for (importer_idx, import) in named_imports {
        if !specifier_needs_interop(&import.imported) {
          continue;
        }
        if link_output.module_table[*importer_idx]
          .as_normal()
          .is_some_and(NormalModule::should_consider_node_esm_spec_for_static_import)
        {
          has_node_mode = true;
        } else {
          has_non_node_mode = true;
        }
        if has_node_mode && has_non_node_mode {
          break;
        }
      }
      if has_node_mode && has_non_node_mode {
        let ext =
          link_output.module_table[*ext_idx].as_external().expect("Should be external module here");
        let canonical_ref = link_output.symbol_db.canonical_ref_for(ext.namespace_ref);
        let original_name = canonical_ref.name(&link_output.symbol_db);
        let node_name = renamer.create_conflictless_name(original_name);
        node_mode_names.insert(canonical_ref, node_name);
      }
    }
    chunk.node_mode_external_ns_names = node_mode_names;
  }

  rename_shadowing_symbols_in_nested_scopes(chunk, link_output, format, &mut renamer);

  chunk.canonical_names = renamer.into_canonical_names();
}

/// Collect the canonical names of things that are emitted at the chunk's root scope and thus
/// captured by every CJS-wrapped module's `__commonJS((exports, module) => { ... })` closure.
/// A real-AST root-scope binding inside the closure whose name matches one of these would
/// shadow the captured value at runtime (issues #9055, #9375, #9630). We track only
/// rolldown-emitted names — iife/umd factory params and `require_xxx` wrapper facades — and
/// intentionally exclude the names of import bindings that get rewritten away at codegen time.
/// We use the symbols' *original* names here: wrapper symbols haven't been renamed yet at this
/// point, and if any of them ends up renamed in the deconfliction loop, the conflict that
/// triggered the rename would have been the user-source local — which is exactly the case we
/// want to catch.
fn collect_chunk_scope_captured_names(
  chunk_idx: ChunkIdx,
  chunk: &Chunk,
  link_output: &LinkStageOutput,
  order_wrap_state: &OrderWrapState,
  order_live_symbols: &FxHashSet<SymbolRef>,
  format: OutputFormat,
  renamer: &Renamer<'_>,
) -> FxHashSet<CompactStr> {
  let mut captured: FxHashSet<CompactStr> = FxHashSet::default();
  for synthetic in order_wrap_state.synthetic_statements_for_chunk(chunk_idx) {
    for declared in &synthetic.declared_symbols {
      captured.insert(CompactStr::new(declared.inner().name(&link_output.symbol_db)));
    }
  }
  if matches!(format, OutputFormat::Iife | OutputFormat::Umd) {
    // Mirror the set rendered as factory params by `render_chunk_external_imports` +
    // `render_factory_parameters`.
    for (external_idx, _) in &chunk.direct_imports_from_external_modules {
      let Some(external) = link_output.module_table[*external_idx].as_external() else {
        continue;
      };
      if let Some(name) = renamer.get_canonical_name(external.namespace_ref) {
        captured.insert(name.clone());
      }
    }
  }
  // CJS wrapper facades (e.g. `require_foo`) are rendered at chunk scope and captured by every
  // CJS-wrapped module's closure in this chunk.
  for module_idx in chunk.modules.iter().copied() {
    if let Some(wrapper_ref) = link_output.metas[module_idx].wrapper_ref {
      let canonical_ref = link_output.symbol_db.canonical_ref_for(wrapper_ref);
      captured.insert(CompactStr::new(canonical_ref.name(&link_output.symbol_db)));
    }
  }
  if matches!(format, OutputFormat::Esm) {
    // A CJS wrapper whose module lives in *another* chunk is hoisted here as a real root-scope
    // import binding (`import { ... as require_foo } from "./other.js"`) and is likewise captured
    // by every CJS-wrapped closure in this chunk. The in-chunk loop above can't see it because its
    // owner module isn't in `chunk.modules`, so we recover it from the cross-chunk import list.
    // Without this, an author-local of the same name inside a CJS closure shadows the imported
    // wrapper, emitting the self-referential `var require_foo = require_foo()` (issue #9630).
    for item in chunk.imports_from_other_chunks.values().flatten() {
      let canonical_ref = link_output.symbol_db.canonical_ref_for(item.import_ref);
      let is_cjs_wrapper =
        link_output.metas[canonical_ref.owner].wrapper_ref.is_some_and(|wrapper_ref| {
          link_output.symbol_db.canonical_ref_for(wrapper_ref) == canonical_ref
        });
      if is_cjs_wrapper || order_live_symbols.contains(&canonical_ref) {
        captured.insert(CompactStr::new(canonical_ref.name(&link_output.symbol_db)));
      }
    }
  }
  captured
}

/// Rename nested scope symbols that would shadow top-level symbols.
///
/// Since we avoid conflicting names during root scope renaming, most nested scope
/// symbols can keep their original names. However, we still need to handle cases
/// where a nested binding would capture a reference to a top-level symbol.
fn rename_shadowing_symbols_in_nested_scopes<'a>(
  chunk: &Chunk,
  link_output: &'a LinkStageOutput,
  output_format: OutputFormat,
  renamer: &mut Renamer<'a>,
) {
  // Same as above, starts with entry module to give entry module symbols naming priority.
  for module_idx in chunk.modules.iter().copied().rev() {
    let Some(module) = link_output.module_table[module_idx].as_normal() else {
      continue;
    };
    let Some(db) = &link_output.symbol_db[module_idx] else {
      continue;
    };

    let mut ctx = NestedScopeRenamer {
      module_idx,
      module,
      db,
      scoping: db.ast_scopes.scoping(),
      link_output,
      renamer,
    };

    ctx.rename_bindings_shadowing_star_imports();
    ctx.rename_bindings_shadowing_named_imports();
    ctx.rename_bindings_shadowing_wrapper_params(matches!(
      output_format,
      OutputFormat::Iife | OutputFormat::Umd | OutputFormat::Cjs
    ));
    ctx.rename_cjs_locals_shadowing_referenced_chunk_bindings();
  }
}
