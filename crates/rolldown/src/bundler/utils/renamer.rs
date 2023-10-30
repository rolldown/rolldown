use std::borrow::Cow;

use oxc::span::Atom;
use rolldown_common::SymbolRef;
use rustc_hash::FxHashMap;

use crate::bundler::graph::symbols::Symbols;

#[derive(Debug)]
pub struct Renamer<'name> {
  name_to_count: FxHashMap<Cow<'name, Atom>, u32>,
  canonical_names: FxHashMap<SymbolRef, Atom>,
  symbols: &'name Symbols,
}

impl<'name> Renamer<'name> {
  pub fn new(symbols: &'name Symbols) -> Self {
    Self { name_to_count: FxHashMap::default(), canonical_names: FxHashMap::default(), symbols }
  }

  pub fn inc(&mut self, name: Cow<'name, Atom>) {
    *self.name_to_count.entry(name).or_default() += 1;
  }

  pub fn add_top_level_symbol(&mut self, symbol_ref: SymbolRef) {
    let canonical_ref = self.symbols.par_canonical_ref_for(symbol_ref);
    let original_name = self.symbols.get_original_name(canonical_ref);

    match self.canonical_names.entry(canonical_ref) {
      std::collections::hash_map::Entry::Occupied(_) => {}
      std::collections::hash_map::Entry::Vacant(vacant) => {
        let count = self.name_to_count.entry(Cow::Borrowed(original_name)).or_default();
        if *count == 0 {
          vacant.insert(original_name.clone());
        } else {
          vacant.insert(format!("{}${}", original_name, *count).into());
        }
        *count += 1;
      }
    }
  }

  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, Atom> {
    self.canonical_names
  }
}
