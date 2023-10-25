use index_vec::IndexVec;
use oxc::{
  semantic::{ReferenceId, SymbolId, SymbolTable},
  span::Atom,
};
use rolldown_common::{ModuleId, SymbolRef};
use rolldown_utils::reserved_word::is_reserved_word;
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct Symbol {
  pub name: Atom,
  /// The symbol that this symbol is linked to.
  pub link: Option<SymbolRef>,
}

#[derive(Debug, Default)]
pub struct SymbolMap {
  pub names: IndexVec<SymbolId, Atom>,
  pub references: IndexVec<ReferenceId, Option<SymbolId>>,
}

impl SymbolMap {
  pub fn from_symbol_table(table: SymbolTable) -> Self {
    Self {
      names: table.names,
      references: table.references.iter().map(oxc::semantic::Reference::symbol_id).collect(),
    }
  }

  pub fn create_symbol(&mut self, name: Atom) -> SymbolId {
    if is_reserved_word(&name) {
      self.names.push(format!("_{name}").into())
    } else {
      self.names.push(name)
    }
  }
}

// Information about symbols for all modules
#[derive(Debug, Default)]
pub struct Symbols {
  inner: IndexVec<ModuleId, IndexVec<SymbolId, Symbol>>,
  pub(crate) references_table: IndexVec<ModuleId, IndexVec<ReferenceId, Option<SymbolId>>>,
}

impl Symbols {
  pub fn new(tables: IndexVec<ModuleId, SymbolMap>) -> Self {
    let mut reference_table = IndexVec::with_capacity(tables.len());
    let inner = tables
      .into_iter()
      .map(|table| {
        reference_table.push(table.references);
        table.names.into_iter().map(|name| Symbol { name, link: None }).collect()
      })
      .collect();

    Self { inner, references_table: reference_table }
  }

  pub fn create_symbol(&mut self, owner: ModuleId, name: Atom) -> SymbolRef {
    let symbol_id = self.inner[owner].push(Symbol { name, link: None });
    SymbolRef { owner, symbol: symbol_id }
  }

  /// Make a point to b
  pub fn union(&mut self, a: SymbolRef, b: SymbolRef) {
    // a link to b
    let root_a = self.get_canonical_ref(a);
    let root_b = self.get_canonical_ref(b);
    if root_a == root_b {
      return;
    }
    self.get_mut(root_a).link = Some(root_b);
  }

  pub fn get_original_name(&self, refer: SymbolRef) -> &Atom {
    &self.get(refer).name
  }

  pub fn get(&self, refer: SymbolRef) -> &Symbol {
    &self.inner[refer.owner][refer.symbol]
  }

  pub fn get_mut(&mut self, refer: SymbolRef) -> &mut Symbol {
    &mut self.inner[refer.owner][refer.symbol]
  }

  pub fn get_canonical_ref(&mut self, target: SymbolRef) -> SymbolRef {
    let canonical = self.par_get_canonical_ref(target);
    if target != canonical {
      // update the link to the canonical so that the next time we can get the canonical directly
      self.get_mut(target).link = Some(canonical);
    }
    canonical
  }

  // Used for the situation where rust require `&self`
  pub fn par_get_canonical_ref(&self, target: SymbolRef) -> SymbolRef {
    let mut canonical = target;
    while let Some(founded) = self.get(canonical).link {
      debug_assert!(founded != target);
      canonical = founded;
    }
    canonical
  }
}

pub fn get_symbol_final_name<'a>(
  symbol: SymbolRef,
  symbols: &'a Symbols,
  final_names: &'a FxHashMap<SymbolRef, Atom>,
) -> Option<&'a Atom> {
  let final_ref = symbols.par_get_canonical_ref(symbol);
  final_names.get(&final_ref)
}
