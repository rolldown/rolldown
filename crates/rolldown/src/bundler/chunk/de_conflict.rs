use std::borrow::Cow;

use super::chunk::Chunk;
use crate::bundler::{graph::graph::Graph, module::Module, utils::renamer::Renamer};

impl Chunk {
  pub fn de_conflict(&mut self, graph: &Graph) {
    let mut renamer = Renamer::new(&graph.symbols);

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
        renamer.inc(name);
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
            .flat_map(|part| part.declared_symbols.iter().copied())
            .chain(
              graph.linking_infos[module.id]
                .facade_stmt_infos
                .iter()
                .flat_map(|part| part.declared_symbols.iter().copied()),
            )
            .for_each(|symbol_ref| {
              renamer.add_top_level_symbol(symbol_ref);
            });
        }
        Module::External(_) => {}
      });

    self.canonical_names = renamer.into_canonical_names();
  }
}
