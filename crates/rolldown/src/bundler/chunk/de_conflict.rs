use std::borrow::Cow;

use oxc::span::Atom;
use rolldown_common::SymbolRef;
use rustc_hash::FxHashMap;

use super::chunk::Chunk;
use crate::bundler::{graph::graph::Graph, module::module::Module};

impl Chunk {
  pub fn de_conflict(&mut self, graph: &Graph) {
    let mut name_to_count = self
      .modules
      .iter()
      .copied()
      .map(|id| &graph.modules[id])
      .filter_map(|m| match m {
        Module::Normal(m) => Some(m.scope.root_unresolved_references().keys().map(Cow::Borrowed)),
        Module::External(_) => None,
      })
      .flatten()
      .map(|name| (name, 1u32))
      .collect::<FxHashMap<_, _>>();

    let mut canonical_names: FxHashMap<SymbolRef, Atom> = FxHashMap::default();

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
              graph.linker_modules[module.id]
                .virtual_stmt_infos
                .iter()
                .flat_map(|part| part.declared_symbols.iter().copied()),
            )
            .for_each(|symbol_id| {
              let canonical_ref =
                graph.symbols.par_get_canonical_ref((module.id, symbol_id).into());

              let original_name = graph.symbols.get_original_name(canonical_ref);

              match canonical_names.entry(canonical_ref) {
                std::collections::hash_map::Entry::Occupied(_) => {}
                std::collections::hash_map::Entry::Vacant(vacant) => {
                  let count = name_to_count.entry(Cow::Borrowed(original_name)).or_default();
                  if *count == 0 {
                    vacant.insert(original_name.clone());
                  } else {
                    vacant.insert(format!("{}${}", original_name, *count).into());
                  }
                  *count += 1;
                }
              }
            });
        }
        Module::External(_) => {}
      });

    self.canonical_names = canonical_names;
  }
}
