use oxc::span::CompactStr;

use crate::{
  stages::link_stage::LinkStageOutput,
  utils::renamer::{NestedScopeRenamer, Renamer},
};
use arcstr::ArcStr;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, GetLocalDb, OutputFormat, TaggedSymbolRef, WrapKind,
};
use rolldown_utils::ecmascript::legitimize_identifier_name;
use rustc_hash::FxHashMap;

#[tracing::instrument(level = "trace", skip_all)]
pub fn deconflict_chunk_symbols(
  chunk: &mut Chunk,
  link_output: &LinkStageOutput,
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
        if link_output.used_symbol_refs.contains(&symbol_ref) {
          renamer.add_symbol_in_root_scope(symbol_ref, true);
        }
      });
      for symbol_id in db.ast_scopes.facade_symbol_classic_data().keys() {
        let symbol_ref = (*module, *symbol_id).into();
        if link_output.used_symbol_refs.contains(&symbol_ref) {
          renamer.add_symbol_in_root_scope(symbol_ref, true);
        }
      }
    });
  }

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

      module
        .stmt_infos
        .iter_enumerated()
        .filter(|(idx, _)| meta.stmt_info_included[*idx])
        .for_each(|(_, stmt_info)| {
          for declared_symbol in stmt_info
            .declared_symbols
            .iter()
            .filter(|item| matches!(item, TaggedSymbolRef::Normal(_)))
          {
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
              link_output.symbol_db.is_facade_symbol(canonical_ref)
                || stmt_info.import_records.iter().any(|import_rec_idx| {
                  module.import_records[*import_rec_idx]
                    .resolved_module
                    .is_some_and(|module_idx| link_output.module_table[module_idx].is_external())
                })
            } else {
              true
            };
            renamer.add_symbol_in_root_scope(symbol_ref, needs_deconflict);
          }
        });
    });

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
        renamer.create_conflictless_name(&legitimize_identifier_name(&format!(
          "require_{}",
          index_chunk_id_to_name[id]
        ))),
      )
    })
    .collect();

  rename_shadowing_symbols_in_nested_scopes(chunk, link_output, format, &mut renamer);

  chunk.canonical_names = renamer.into_canonical_names();
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
  }
}
