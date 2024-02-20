use std::borrow::Cow;

use super::chunk::Chunk;
use crate::bundler::{
  module::Module, stages::link_stage::LinkStageOutput, utils::renamer::Renamer,
};

impl Chunk {
  pub fn de_conflict(&mut self, graph: &LinkStageOutput) {
    let mut renamer = Renamer::new(&graph.symbols, graph.modules.len());

    // TODO: reserve names for keywords in both non-strict and strict mode

    self
      .modules
      .iter()
      .copied()
      .map(|id| &graph.modules[id])
      .filter_map(|m| match m {
        Module::Normal(m) => Some(m.scope.root_unresolved_references().keys().map(Cow::Borrowed)),
        Module::External(_) => None,
      })
      .flatten()
      .for_each(|name| {
        // global names should be reserved
        renamer.reserve(name);
      });

    self.imports_from_other_chunks.iter().flat_map(|(_, items)| items.iter()).for_each(|item| {
      renamer.add_top_level_symbol(item.import_ref);
    });

    self
      .modules
      .iter()
      .copied()
      // Starts with entry module
      .rev()
      .map(|id| &graph.modules[id])
      .for_each(|module| match module {
        Module::Normal(module) => {
          module
            .stmt_infos
            .iter()
            .filter(|stmt_info| stmt_info.is_included)
            .flat_map(|stmt_info| stmt_info.declared_symbols.iter().copied())
            .for_each(|symbol_ref| {
              renamer.add_top_level_symbol(symbol_ref);
            });
        }
        Module::External(_) => {}
      });

    // rename non-top-level names
    renamer.rename_non_top_level_symbol(&self.modules, &graph.modules);

    self.canonical_names = renamer.into_canonical_names();
  }
}
