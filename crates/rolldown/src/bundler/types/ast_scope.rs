use index_vec::IndexVec;
use oxc::semantic::{Reference, ReferenceId, ScopeTree, SymbolId};

#[derive(Debug)]
pub struct AstScope {
  inner: ScopeTree,
  references: IndexVec<ReferenceId, Reference>,
}

impl AstScope {
  pub fn new(inner: ScopeTree, references: IndexVec<ReferenceId, Reference>) -> Self {
    Self { inner, references }
  }

  pub fn is_unresolved(&self, reference_id: ReferenceId) -> bool {
    self.references[reference_id].symbol_id().is_none()
  }

  pub fn symbol_id_for(&self, reference_id: ReferenceId) -> Option<SymbolId> {
    self.references[reference_id].symbol_id()
  }
}

impl std::ops::Deref for AstScope {
  type Target = ScopeTree;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}
