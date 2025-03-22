use oxc::semantic::{Reference, ReferenceId, Scoping, SymbolId};

#[derive(Debug)]
pub struct AstScopes {
  scoping: Scoping,
}
impl Clone for AstScopes {
  fn clone(&self) -> Self {
    // FIXME: use real clone, need upstream support
    Self { scoping: Scoping::default() }
  }
}

impl AstScopes {
  pub fn new(inner: Scoping) -> Self {
    Self { scoping: inner }
  }

  pub fn is_unresolved(&self, reference_id: ReferenceId) -> bool {
    self.get_reference(reference_id).symbol_id().is_none()
  }

  pub fn symbol_id_for(&self, reference_id: ReferenceId) -> Option<SymbolId> {
    self.scoping.get_reference(reference_id).symbol_id()
  }

  pub fn get_resolved_references(&self, symbol_id: SymbolId) -> impl Iterator<Item = &Reference> {
    self.scoping.get_resolved_references(symbol_id)
  }
}

impl std::ops::Deref for AstScopes {
  type Target = Scoping;

  fn deref(&self) -> &Self::Target {
    &self.scoping
  }
}

impl std::ops::DerefMut for AstScopes {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.scoping
  }
}
