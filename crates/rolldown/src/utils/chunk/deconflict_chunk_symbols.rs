use std::borrow::Cow;

use rolldown_rstr::ToRstr;

use crate::{chunk::Chunk, stages::link_stage::LinkStageOutput, utils::renamer::Renamer};

pub fn deconflict_chunk_symbols(chunk: &mut Chunk, link_output: &LinkStageOutput) {
  let mut renamer =
    Renamer::new(&link_output.symbols, link_output.module_table.normal_modules.len());

  chunk
    .modules
    .iter()
    .copied()
    .map(|id| &link_output.module_table.normal_modules[id])
    .flat_map(|m| m.scope.root_unresolved_references().keys().map(Cow::Borrowed))
    .for_each(|name| {
      // global names should be reserved
      renamer.reserve(Cow::Owned(name.to_rstr()));
    });

  chunk.imports_from_other_chunks.iter().flat_map(|(_, items)| items.iter()).for_each(|item| {
    renamer.add_top_level_symbol(item.import_ref);
  });

  chunk
    .modules
    .iter()
    .copied()
    // Starts with entry module
    .rev()
    .map(|id| &link_output.module_table.normal_modules[id])
    .for_each(|module| {
      module
        .stmt_infos
        .iter()
        .filter(|stmt_info| stmt_info.is_included)
        .flat_map(|stmt_info| stmt_info.declared_symbols.iter().copied())
        .for_each(|symbol_ref| {
          renamer.add_top_level_symbol(symbol_ref);
        });
    });

  // rename non-top-level names
  renamer.rename_non_top_level_symbol(&chunk.modules, &link_output.module_table.normal_modules);

  chunk.canonical_names = renamer.into_canonical_names();
}
