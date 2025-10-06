use oxc::semantic::{Reference, ReferenceId, ScopeId, Scoping, SymbolId};
use rustc_hash::FxHashMap;

use super::symbol_ref_db::SymbolRefDataClassic;
/// 0             | minimum_symbol_id                          u32::MAX
/// ┌──────────────────────────────────────────────────────────┐
/// │                                                          │
/// └──────────────────────────────────────────────────────────┘
///
/// ──────────────►                          ◄─────────────────
/// scoping_symbols                                             facade_scoping_symbols
///
/// The basic idea is use a pivot to prevent facade symbols to override real symbols data.
/// This could avoid mutate real symbols in scoping
#[derive(Debug, Clone)]
pub struct FacadeScoping {
  // the minimum symbol id that could be used by the facade, if less than this, two symbol table
  // will be overlapped
  minimum_symbol_id: SymbolId,
  next_symbol_id: u32,
  mutated_symbol_id_to_names: FxHashMap<SymbolId, String>,
  pub(crate) facade_symbol_classic_data: FxHashMap<SymbolId, SymbolRefDataClassic>,
}

#[derive(Debug)]
pub struct AstScopes {
  scoping: Scoping,
  pub(crate) facade_scoping: FacadeScoping,
}

impl AstScopes {
  pub fn new(inner: Scoping) -> Self {
    let facade_scoping = FacadeScoping {
      minimum_symbol_id: SymbolId::from_usize(inner.symbols_len()),
      next_symbol_id: u32::MAX - 1,
      mutated_symbol_id_to_names: FxHashMap::default(),
      facade_symbol_classic_data: FxHashMap::default(),
    };
    Self { scoping: inner, facade_scoping }
  }

  #[inline]
  pub fn scoping(&self) -> &Scoping {
    &self.scoping
  }

  pub fn into_inner(self) -> (Scoping, FacadeScoping) {
    (self.scoping, self.facade_scoping)
  }

  pub fn set_scoping(&mut self, scoping: Scoping) {
    self.scoping = scoping;
  }

  pub fn set_facade_scope(&mut self, facade_scoping: FacadeScoping) {
    self.facade_scoping = facade_scoping;
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

  #[expect(clippy::cast_possible_truncation)]
  pub fn create_facade_root_symbol_ref(&mut self, name: &str) -> SymbolId {
    assert!(
      self.facade_scoping.next_symbol_id > self.facade_scoping.minimum_symbol_id.index() as u32,
    );
    let symbol_id = self.facade_scoping.next_symbol_id;
    self.facade_scoping.next_symbol_id -= 1;
    self
      .facade_scoping
      .mutated_symbol_id_to_names
      .insert(SymbolId::from_raw_unchecked(symbol_id), name.to_string());
    self
      .facade_scoping
      .facade_symbol_classic_data
      .insert(SymbolId::from_raw_unchecked(symbol_id), SymbolRefDataClassic::default());
    SymbolId::from_raw_unchecked(symbol_id)
  }

  pub fn set_symbol_name(&mut self, symbol_id: SymbolId, name: &str) {
    self.facade_scoping.mutated_symbol_id_to_names.insert(symbol_id, name.to_string());
  }

  pub fn symbol_name(&self, symbol_id: SymbolId) -> &str {
    self
      .facade_scoping
      .mutated_symbol_id_to_names
      .get(&symbol_id)
      .map_or_else(|| self.scoping.symbol_name(symbol_id), std::string::String::as_str)
  }

  #[inline]
  pub fn is_facade_symbol(&self, symbol_id: SymbolId) -> bool {
    symbol_id.index() >= self.facade_scoping.minimum_symbol_id.index()
  }

  #[inline]
  pub fn real_symbol_length(&self) -> usize {
    self.scoping.symbols_len()
  }

  #[inline]
  pub fn get_symbol_classic_data(&self, symbol_id: SymbolId) -> Option<&SymbolRefDataClassic> {
    self.facade_scoping.facade_symbol_classic_data.get(&symbol_id)
  }

  #[inline]
  pub fn symbol_scope_id(&self, symbol_id: SymbolId) -> ScopeId {
    if symbol_id < self.facade_scoping.minimum_symbol_id {
      self.scoping.symbol_scope_id(symbol_id)
    } else {
      self.scoping.root_scope_id()
    }
  }

  pub fn facade_symbol_classic_data(&self) -> &FxHashMap<SymbolId, SymbolRefDataClassic> {
    &self.facade_scoping.facade_symbol_classic_data
  }

  #[must_use]
  pub fn clone_facade_only(&self) -> AstScopes {
    AstScopes { scoping: Scoping::default(), facade_scoping: self.facade_scoping.clone() }
  }
}
