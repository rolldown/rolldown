use std::borrow::Cow;

use crate::{stages::link_stage::LinkStageOutput, utils::renamer::Renamer};
use arcstr::ArcStr;
use oxc::ast::ast::TaggedTemplateExpression;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, GetLocalDb, ModuleScopeSymbolIdMap, OutputFormat, StmtSideEffect,
  TaggedSymbolRef,
};
use rolldown_rstr::ToRstr;
use rolldown_utils::{
  ecmascript::legitimize_identifier_name,
  global_reference::is_side_effect_free_member_expr_of_len_three,
};
use rustc_hash::FxHashMap;

#[allow(clippy::too_many_lines)]
#[tracing::instrument(level = "trace", skip_all)]
pub fn deconflict_chunk_symbols(
  chunk: &mut Chunk,
  link_output: &LinkStageOutput,
  format: OutputFormat,
  index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
  map: &ModuleScopeSymbolIdMap<'_>,
) {
  let mut renamer = Renamer::new(&link_output.symbol_db, format);

  if matches!(format, OutputFormat::Iife | OutputFormat::Umd | OutputFormat::Cjs) {
    // deconflict iife introduce symbols by external
    // Also AMD, but we don't support them yet.
    chunk
      .imports_from_external_modules
      .iter()
      .filter_map(|(idx, _)| link_output.module_table[*idx].as_external())
      .for_each(|external_module| {
        renamer.add_symbol_in_root_scope(external_module.namespace_ref);
      });

    match chunk.entry_module_idx() {
      Some(module) => {
        let entry_module =
          link_output.module_table[module].as_normal().expect("should be normal module");
        link_output.metas[entry_module.idx].star_exports_from_external_modules.iter().for_each(
          |rec_idx| {
            let rec = &entry_module.ecma_view.import_records[*rec_idx];
            let external_module = &link_output.module_table[rec.resolved_module]
              .as_external()
              .expect("Should be external module here");
            renamer.add_symbol_in_root_scope(external_module.namespace_ref);
          },
        );
      }
      None => {}
    }
  }

  chunk
    .modules
    .iter()
    .copied()
    .filter_map(|id| link_output.module_table[id].as_normal())
    .flat_map(|m| {
      link_output.symbol_db[m.idx]
        .as_ref()
        .unwrap()
        .ast_scopes
        .scoping()
        .root_unresolved_references()
        .keys()
        .map(Cow::Borrowed)
    })
    .for_each(|name| {
      // global names should be reserved
      renamer.reserve(name.to_rstr());
    });

  // Though, those symbols in `imports_from_other_chunks` doesn't belong to this chunk, but in the final output, they still behave
  // like declared in this chunk. This is because we need to generate import statements in this chunk to import symbols from other
  // statements. Those `import {...} from './other-chunk.js'` will declared these outside symbols in this chunk, so symbols that
  // point to them can be resolved in runtime.
  // So we add them in the deconflict process to generate conflict-less names in this chunk.
  chunk.imports_from_other_chunks.iter().flat_map(|(_, items)| items.iter()).for_each(|item| {
    renamer.add_symbol_in_root_scope(item.import_ref);
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

  match chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      let meta = &link_output.metas[module];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|(symbol_ref, is_facade)| {
        if !is_facade {
          renamer.add_symbol_in_root_scope(*symbol_ref);
        }
      });
    }
    ChunkKind::Common => {}
  }
  if matches!(format, OutputFormat::Esm) {
    chunk.imports_from_external_modules.iter().for_each(|(module, _)| {
      let db = link_output.symbol_db.local_db(*module);
      db.classic_data.iter_enumerated().for_each(|(symbol, _)| {
        let symbol_ref = (*module, symbol).into();
        if link_output.used_symbol_refs.contains(&symbol_ref) {
          renamer.add_symbol_in_root_scope(symbol_ref);
        }
      });
      for symbol_id in db.ast_scopes.facade_symbol_classic_data().keys() {
        let symbol_ref = (*module, *symbol_id).into();
        if link_output.used_symbol_refs.contains(&symbol_ref) {
          renamer.add_symbol_in_root_scope(symbol_ref);
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
        renamer.add_symbol_in_root_scope(hmr_hot_ref);
      }

      module
        .stmt_infos
        .iter()
        .filter(|stmt_info| stmt_info.is_included)
        .flat_map(|stmt_info| {
          stmt_info
            .declared_symbols
            .iter()
            .filter(|item| matches!(item, TaggedSymbolRef::Normal(_)))
            .copied()
        })
        .for_each(|symbol_ref| {
          if symbol_ref.inner().owner != 0 {
            dbg!(&symbol_ref);
            dbg!(&symbol_ref.inner().name(&link_output.symbol_db));
          }
          renamer.add_symbol_in_root_scope(symbol_ref.inner());
        });
    });

  // rename non-top-level names
  renamer.rename_non_root_symbol(&chunk.modules, link_output, map);

  chunk.canonical_names = renamer.into_canonical_names();
}
