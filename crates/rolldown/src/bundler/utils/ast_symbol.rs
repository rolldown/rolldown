use index_vec::IndexVec;
use oxc::{
  semantic::{ScopeId, SymbolId, SymbolTable},
  span::Atom,
};

#[derive(Debug, Default)]
pub struct AstSymbol {
  pub names: IndexVec<SymbolId, Atom>,
  pub scope_ids: IndexVec<SymbolId, ScopeId>,
}

impl AstSymbol {
  pub fn from_symbol_table(table: SymbolTable) -> Self {
    debug_assert!(table.references.is_empty());
    Self { names: table.names, scope_ids: table.scope_ids }
  }

  pub fn create_symbol(&mut self, name: Atom, scope_id: ScopeId) -> SymbolId {
    self.scope_ids.push(scope_id);
    self.names.push(name)
  }

  pub fn scope_id_for(&self, symbol_id: SymbolId) -> ScopeId {
    self.scope_ids[symbol_id]
  }
}
