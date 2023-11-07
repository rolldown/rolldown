use std::borrow::Cow;

use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::bundler::graph::symbols::Symbols;

#[derive(Debug)]
pub struct Renamer<'name> {
  top_level_name_to_count: FxHashMap<Cow<'name, Atom>, u32>,
  used_canonical_names: FxHashSet<Cow<'name, Atom>>,
  module_to_used_canonical_name_count: IndexVec<ModuleId, FxHashMap<Cow<'name, Atom>, u32>>,
  canonical_names: FxHashMap<SymbolRef, Atom>,
  symbols: &'name Symbols,
}

impl<'name> Renamer<'name> {
  pub fn new(symbols: &'name Symbols, modules_len: usize) -> Self {
    Self {
      top_level_name_to_count: FxHashMap::default(),
      canonical_names: FxHashMap::default(),
      symbols,
      used_canonical_names: FxHashSet::default(),
      module_to_used_canonical_name_count: index_vec::index_vec![FxHashMap::default(); modules_len],
    }
  }

  pub fn inc(&mut self, name: Cow<'name, Atom>) {
    *self.top_level_name_to_count.entry(name).or_default() += 1;
  }

  pub fn add_top_level_symbol(&mut self, symbol_ref: SymbolRef) {
    let canonical_ref = self.symbols.par_canonical_ref_for(symbol_ref);
    let original_name = Cow::Borrowed(self.symbols.get_original_name(canonical_ref));

    match self.canonical_names.entry(canonical_ref) {
      std::collections::hash_map::Entry::Occupied(_) => {
        // The symbol is already renamed
      }
      std::collections::hash_map::Entry::Vacant(vacant) => {
        let count = self.top_level_name_to_count.entry(original_name.clone()).or_default();
        let canonical_name = if *count == 0 {
          original_name
        } else {
          Cow::Owned(format!("{}${}", original_name, *count).into())
        };
        self.used_canonical_names.insert(canonical_name.clone());
        vacant.insert(canonical_name.into_owned());
        *count += 1;
      }
    }
  }

  // non-top-level symbols won't be linked cross-module. So the canonical `SymbolRef` for them are themselves.
  pub fn add_non_top_level_symbol(&mut self, module_id: ModuleId, canonical_ref: SymbolRef) {
    let original_name = Cow::Borrowed(self.symbols.get_original_name(canonical_ref));

    match self.canonical_names.entry(canonical_ref) {
      std::collections::hash_map::Entry::Occupied(_) => {
        // The symbol is already renamed
      }
      std::collections::hash_map::Entry::Vacant(vacant) => {
        let might_shadowed = self.used_canonical_names.contains(&original_name);
        if might_shadowed {
          let used_canonical_name_count = &mut self.module_to_used_canonical_name_count[module_id];
          // The name is already used in top level, so the default count is 1
          let count = used_canonical_name_count.entry(original_name.clone()).or_insert(1);
          let canonical_name = format!("{}${}", original_name, *count);
          vacant.insert(canonical_name.into());
          *count += 1;
        }
      }
    }
  }

  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, Atom> {
    self.canonical_names
  }
}
