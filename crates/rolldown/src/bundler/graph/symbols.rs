use index_vec::IndexVec;
use oxc::{
  semantic::{ReferenceId, SymbolId, SymbolTable},
  span::Atom,
};
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashMap;

#[derive(Debug, Default)]
pub struct SymbolMap {
  pub names: IndexVec<SymbolId, Atom>,
  pub references: IndexVec<ReferenceId, Option<SymbolId>>,
}

impl SymbolMap {
  pub fn from_symbol_table(table: SymbolTable) -> Self {
    Self {
      names: table.names,
      references: table
        .references
        .iter()
        .map(|refer| refer.symbol_id())
        .collect(),
    }
  }

  pub fn create_symbol(&mut self, name: Atom) -> SymbolId {
    self.names.push(name)
  }

  pub fn create_reference(&mut self, id: Option<SymbolId>) -> ReferenceId {
    self.references.push(id)
  }

  pub fn get_name(&self, id: SymbolId) -> &Atom {
    &self.names[id]
  }
}

// Information about symbols for all modules
#[derive(Debug, Default)]
pub struct Symbols {
  pub(crate) tables: IndexVec<ModuleId, SymbolMap>,
  canonical_refs: IndexVec<ModuleId, FxHashMap<SymbolId, SymbolRef>>,
}

impl Symbols {
  pub fn new(tables: IndexVec<ModuleId, SymbolMap>) -> Self {
    Self {
      canonical_refs: tables.iter().map(|_table| FxHashMap::default()).collect(),
      tables,
    }
  }

  /// Make a point to b
  pub fn union(&mut self, a: SymbolRef, b: SymbolRef) {
    // a link to b
    let root_a = self.get_canonical_ref(a);
    let root_b = self.get_canonical_ref(b);
    if root_a == root_b {
      return;
    }
    self.canonical_refs[a.owner].insert(a.symbol, root_b);
  }

  pub fn get_original_name(&self, refer: SymbolRef) -> &Atom {
    self.tables[refer.owner].get_name(refer.symbol)
  }

  pub fn get_canonical_ref(&mut self, target: SymbolRef) -> SymbolRef {
    let mut canonical = target;
    while let Some(founded) = self.canonical_refs[canonical.owner]
      .get(&canonical.symbol)
      .copied()
    {
      debug_assert!(founded != target);
      canonical = founded;
    }
    if target != canonical {
      self.canonical_refs[target.owner].insert(target.symbol, canonical);
    }
    canonical
  }

  pub fn par_get_canonical_ref(&self, target: SymbolRef) -> SymbolRef {
    let mut canonical = target;
    while let Some(founded) = self.canonical_refs[canonical.owner]
      .get(&canonical.symbol)
      .copied()
    {
      debug_assert!(founded != canonical);
      canonical = founded;
    }
    canonical
  }
}

pub fn get_symbol_final_name<'a>(
  module_id: ModuleId,
  symbol_id: SymbolId,
  symbols: &'a Symbols,
  final_names: &'a FxHashMap<SymbolRef, Atom>,
) -> Option<&'a Atom> {
  let symbol_ref = (module_id, symbol_id).into();
  let final_ref = symbols.par_get_canonical_ref(symbol_ref);
  final_names.get(&final_ref)
}

pub fn get_reference_final_name<'a>(
  module_id: ModuleId,
  reference_id: ReferenceId,
  symbols: &'a Symbols,
  final_names: &'a FxHashMap<SymbolRef, Atom>,
) -> Option<&'a Atom> {
  symbols.tables[module_id].references[reference_id]
    .and_then(|symbol| get_symbol_final_name(module_id, symbol, symbols, final_names))
}
