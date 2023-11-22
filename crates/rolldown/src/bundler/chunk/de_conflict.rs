use std::borrow::Cow;

use super::chunk::Chunk;
use crate::bundler::{
  module::Module, stages::link_stage::LinkStageOutput, utils::renamer::Renamer,
};

impl Chunk {
  pub fn de_conflict(&mut self, graph: &LinkStageOutput) {
    let mut renamer = Renamer::new(&graph.symbols, graph.modules.len());

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

    // rename non-top-level names

    self.modules.iter().copied().for_each(|module| {
      let Module::Normal(module) = &graph.modules[module] else { return };
      module.scope.descendants().for_each(|scope_id| {
        if scope_id == module.scope.root_scope_id() {
          // Top level symbol are already processed above
          return;
        }
        let bindings = module.scope.get_bindings(scope_id);
        bindings.iter().for_each(|(_binding_name, symbol_id)| {
          renamer.add_non_top_level_symbol(module.id, (module.id, *symbol_id).into());
        });
      });
    });

    self.canonical_names = renamer.into_canonical_names();
  }
}
