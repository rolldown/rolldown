use oxc::semantic::{Reference, ReferenceId, ScopeTree, SymbolId, SymbolTable};

#[derive(Debug)]
pub struct AstScopes {
  inner: ScopeTree,
}

impl AstScopes {
  pub fn new(inner: ScopeTree) -> Self {
    Self { inner }
  }

  pub fn is_unresolved(&self, reference_id: ReferenceId, symbol_table: &SymbolTable) -> bool {
    symbol_table.references[reference_id].symbol_id().is_none()
  }

  pub fn symbol_id_for(
    &self,
    reference_id: ReferenceId,
    symbol_table: &SymbolTable,
  ) -> Option<SymbolId> {
    symbol_table.references[reference_id].symbol_id()
  }

  pub fn get_resolved_references<'table>(
    &self,
    symbol_id: SymbolId,
    symbol_table: &'table SymbolTable,
  ) -> impl Iterator<Item = &'table Reference> + 'table {
    symbol_table.get_resolved_references(symbol_id)
  }
}

impl std::ops::Deref for AstScopes {
  type Target = ScopeTree;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}
