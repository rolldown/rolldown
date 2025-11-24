use oxc::span::CompactStr;

use crate::{
  stages::link_stage::LinkStageOutput,
  utils::{
    chunk::collect_transitive_external_star_exports::collect_transitive_external_star_exports,
    renamer::Renamer,
  },
};
use arcstr::ArcStr;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, GetLocalDb, ModuleScopeSymbolIdMap, OutputFormat, TaggedSymbolRef,
};
use rolldown_utils::ecmascript::legitimize_identifier_name;
use rustc_hash::FxHashMap;

#[tracing::instrument(level = "trace", skip_all)]
pub fn deconflict_chunk_symbols(
  chunk: &mut Chunk,
  link_output: &LinkStageOutput,
  format: OutputFormat,
  index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
  map: &ModuleScopeSymbolIdMap<'_>,
) {
  let mut renamer = Renamer::new(&link_output.symbol_db, format);

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
    })
    .for_each(|name| {
      // global names should be reserved
      renamer.reserve(CompactStr::new(name));
    });

  if matches!(format, OutputFormat::Iife | OutputFormat::Umd | OutputFormat::Cjs) {
    // deconflict iife introduce symbols by external
    // Also AMD, but we don't support them yet.
    chunk
      .direct_imports_from_external_modules
      .iter()
      .filter_map(|(idx, _)| link_output.module_table[*idx].as_external())
      .for_each(|external_module| {
        renamer.add_symbol_in_root_scope(external_module.namespace_ref);
      });

    chunk
      .import_symbol_from_external_modules
      .iter()
      .filter_map(|idx| link_output.module_table[*idx].as_external())
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

        // FIX FOR ISSUE #7115: Also add transitive external star exports
        // This handles cases like: index.js → export * from './server.js' → export * from 'external-lib'
        let transitive_external_star_exports =
          collect_transitive_external_star_exports(entry_module.idx, &link_output.module_table);
        for external_idx in transitive_external_star_exports {
          let external = &link_output.module_table[external_idx]
            .as_external()
            .expect("Should be external module");
          renamer.add_symbol_in_root_scope(external.namespace_ref);
        }
      }
      None => {}
    }
  }

  match chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      let meta = &link_output.metas[module];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(
        |(symbol_ref, came_from_cjs)| {
          if !came_from_cjs {
            renamer.add_symbol_in_root_scope(*symbol_ref);
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
          renamer.add_symbol_in_root_scope(symbol_ref.inner());
        });
    });

  // Though, those symbols in `imports_from_other_chunks` doesn't belong to this chunk, but in the final output, they still behave
  // like declared in this chunk. This is because we need to generate import statements in this chunk to import symbols from other
  // statements. Those `import {...} from './other-chunk.js'` will declared these outside symbols in this chunk, so symbols that
  // point to them can be resolved in runtime.
  // So we add them in the deconflict process to generate conflict-less names in this chunk.
  chunk.imports_from_other_chunks.iter().flat_map(|(_, items)| items.iter()).for_each(|item| {
    renamer.add_symbol_in_root_scope(item.import_ref);
  });

  // Similarly, symbols in `exports_to_other_chunks` need canonical names because they are rendered
  // in the chunk's export statements. We add them to the renamer to ensure they have canonical names.
  chunk.exports_to_other_chunks.keys().for_each(|export_ref| {
    renamer.add_symbol_in_root_scope(*export_ref);
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

  // rename non-top-level names
  renamer.rename_non_root_symbol(&chunk.modules, link_output, map);

  chunk.canonical_names = renamer.into_canonical_names();
}

/// Generates deconflicted binding names for star reexports in preserveModules mode.
/// This must be called after `deconflict_chunk_symbols` has run for all chunks.
///
/// This handles cases like: `export * from './server.js'` when both files are separate chunks.
/// The generated names (e.g., "require_server") are stored in the chunk for use during rendering.
#[tracing::instrument(level = "trace", skip_all)]
pub fn generate_star_reexport_binding_names(
  chunk: &mut Chunk,
  link_output: &LinkStageOutput,
  module_to_chunk: &oxc_index::IndexVec<rolldown_common::ModuleIdx, Option<ChunkIdx>>,
  index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
) {
  let Some(entry_module_idx) = chunk.entry_module_idx() else {
    return;
  };

  let entry_module =
    link_output.module_table[entry_module_idx].as_normal().expect("should be normal module");

  // Collect star-exported modules that are internal/normal modules
  let internal_star_export_chunk_ids: std::collections::BTreeSet<ChunkIdx> = entry_module
    .star_export_module_ids()
    .filter_map(|module_idx| {
      // Only consider normal (internal) modules, not external ones
      match &link_output.module_table[module_idx] {
        rolldown_common::Module::Normal(_) => {
          // Find which chunk this module belongs to
          module_to_chunk.get(module_idx).and_then(|opt| *opt)
        }
        rolldown_common::Module::External(_) => None,
      }
    })
    .collect();

  // Create a renamer initialized with all existing canonical names in this chunk
  // This ensures the generated star reexport binding names don't conflict with existing symbols
  let mut renamer = Renamer::new(&link_output.symbol_db, OutputFormat::Cjs);

  // Reserve all existing canonical names to avoid conflicts
  for name in chunk.canonical_names.values() {
    renamer.reserve(name.clone());
  }

  // Reserve existing require binding names
  for name in chunk.require_binding_names_for_other_chunks.values() {
    renamer.reserve(CompactStr::new(name));
  }

  // Generate conflict-free binding names for star reexports
  chunk.require_binding_names_for_star_reexports = internal_star_export_chunk_ids
    .into_iter()
    .map(|chunk_idx| {
      (
        chunk_idx,
        renamer.create_conflictless_name(&legitimize_identifier_name(&format!(
          "require_{}",
          index_chunk_id_to_name[&chunk_idx]
        ))),
      )
    })
    .collect();
}
