use oxc::semantic::{NodeId, Reference, ReferenceId, ScopeId, Scoping, SymbolFlags, SymbolId};
use oxc::span::SPAN;
use oxc_str::Ident;

#[derive(Debug)]
pub struct AstScopes {
  scoping: Scoping,
}

impl AstScopes {
  pub fn new(inner: Scoping) -> Self {
    Self { scoping: inner }
  }

  #[inline]
  pub fn scoping(&self) -> &Scoping {
    &self.scoping
  }

  pub fn into_scoping(self) -> Scoping {
    self.scoping
  }

  pub fn set_scoping(&mut self, scoping: Scoping) {
    self.scoping = scoping;
  }

  pub fn is_unresolved(&self, reference_id: ReferenceId) -> bool {
    self.scoping.get_reference(reference_id).symbol_id().is_none()
  }

  pub fn symbol_id_for(&self, reference_id: ReferenceId) -> Option<SymbolId> {
    self.scoping.get_reference(reference_id).symbol_id()
  }

  pub fn get_resolved_references(&self, symbol_id: SymbolId) -> impl Iterator<Item = &Reference> {
    self.scoping.get_resolved_references(symbol_id)
  }

  /// Create a facade symbol in the root scope of the Scoping.
  /// Facade symbols are synthetic symbols not present in the original AST.
  pub fn create_facade_root_symbol_ref(&mut self, name: &str) -> SymbolId {
    self.scoping.create_symbol(
      SPAN,
      Ident::from(name),
      SymbolFlags::empty(),
      self.scoping.root_scope_id(),
      NodeId::DUMMY,
    )
  }

  pub fn set_symbol_name(&mut self, symbol_id: SymbolId, name: &str) {
    self.scoping.set_symbol_name(symbol_id, Ident::from(name));
  }

  pub fn symbol_name(&self, symbol_id: SymbolId) -> &str {
    self.scoping.symbol_name(symbol_id)
  }

  #[inline]
  pub fn total_symbol_count(&self) -> usize {
    self.scoping.symbols_len()
  }

  #[inline]
  pub fn symbol_scope_id(&self, symbol_id: SymbolId) -> ScopeId {
    self.scoping.symbol_scope_id(symbol_id)
  }
}
