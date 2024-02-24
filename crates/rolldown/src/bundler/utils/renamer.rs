use std::borrow::Cow;

use oxc::{semantic::ScopeId, span::Atom};
use rolldown_common::{NormalModuleId, SymbolRef};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::bundler::{
  module::{ModuleVec, NormalModule},
  types::symbols::Symbols,
};

#[derive(Debug)]
pub struct Renamer<'name> {
  used_canonical_names: FxHashSet<Cow<'name, Atom>>,
  canonical_names: FxHashMap<SymbolRef, Atom>,
  symbols: &'name Symbols,
}

impl<'name> Renamer<'name> {
  pub fn new(symbols: &'name Symbols, _modules_len: usize) -> Self {
    Self {
      canonical_names: FxHashMap::default(),
      symbols,
      used_canonical_names: FxHashSet::default(),
    }
  }

  pub fn reserve(&mut self, name: Cow<'name, Atom>) {
    self.used_canonical_names.insert(name);
  }

  pub fn add_top_level_symbol(&mut self, symbol_ref: SymbolRef) {
    let canonical_ref = self.symbols.par_canonical_ref_for(symbol_ref);
    let original_name = Cow::Borrowed(self.symbols.get_original_name(canonical_ref));

    match self.canonical_names.entry(canonical_ref) {
      std::collections::hash_map::Entry::Vacant(vacant) => {
        let mut count = 0;
        let mut candidate_name = original_name.clone();
        while self.used_canonical_names.contains(&candidate_name) {
          count += 1;
          candidate_name = Cow::Owned(format!("{original_name}${count}").into());
        }
        self.used_canonical_names.insert(candidate_name.clone());
        vacant.insert(candidate_name.into_owned());
      }
      std::collections::hash_map::Entry::Occupied(_) => {
        // The symbol is already renamed
      }
    }
  }

  // non-top-level symbols won't be linked cross-module. So the canonical `SymbolRef` for them are themselves.
  pub fn rename_non_top_level_symbol(
    &mut self,
    modules_in_chunk: &[NormalModuleId],
    modules: &ModuleVec,
  ) {
    use rayon::prelude::*;

    fn rename_symbols_of_nested_scopes<'name>(
      module: &'name NormalModule,
      scope_id: ScopeId,
      stack: &mut Vec<Cow<FxHashSet<Cow<'name, Atom>>>>,
      canonical_names: &mut FxHashMap<SymbolRef, Atom>,
    ) {
      let bindings = module.scope.get_bindings(scope_id);
      let mut used_canonical_names_for_this_scope = FxHashSet::default();
      used_canonical_names_for_this_scope.shrink_to(bindings.len());
      bindings.iter().for_each(|(binding_name, symbol_id)| {
        used_canonical_names_for_this_scope.insert(Cow::Borrowed(binding_name));
        let binding_ref: SymbolRef = (module.id, *symbol_id).into();

        let mut count = 1;
        let mut candidate_name = Cow::Borrowed(binding_name);
        match canonical_names.entry(binding_ref) {
          std::collections::hash_map::Entry::Vacant(slot) => loop {
            let is_shadowed = stack
              .iter()
              .any(|used_canonical_names| used_canonical_names.contains(&candidate_name));

            if is_shadowed {
              candidate_name = Cow::Owned(format!("{binding_name}${count}").into());
              count += 1;
            } else {
              used_canonical_names_for_this_scope.insert(candidate_name.clone());
              slot.insert(candidate_name.into_owned());
              break;
            }
          },
          std::collections::hash_map::Entry::Occupied(_) => {
            // The symbol is already renamed
          }
        }
      });

      stack.push(Cow::Owned(used_canonical_names_for_this_scope));
      let child_scopes = module.scope.get_child_ids(scope_id).cloned().unwrap_or_default();
      child_scopes.into_iter().for_each(|scope_id| {
        rename_symbols_of_nested_scopes(module, scope_id, stack, canonical_names);
      });
      stack.pop();
    }

    let canonical_names_of_nested_scopes = modules_in_chunk
      .par_iter()
      .copied()
      .filter_map(|id| modules[id].as_normal())
      .flat_map(|module| {
        let child_scopes: &[ScopeId] =
          module.scope.get_child_ids(module.scope.root_scope_id()).map_or(&[], Vec::as_slice);

        child_scopes.into_par_iter().map(|child_scope_id| {
          let mut stack = vec![Cow::Borrowed(&self.used_canonical_names)];
          let mut canonical_names = FxHashMap::default();
          rename_symbols_of_nested_scopes(
            module,
            *child_scope_id,
            &mut stack,
            &mut canonical_names,
          );
          canonical_names
        })
      })
      .reduce(FxHashMap::default, |mut acc, canonical_names| {
        acc.extend(canonical_names);
        acc
      });

    self.canonical_names.extend(canonical_names_of_nested_scopes);
  }

  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, Atom> {
    self.canonical_names
  }
}
