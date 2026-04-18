use rustc_hash::FxHashSet;

use super::symbol_ref::SymbolRef;

#[derive(Debug, Default)]
pub struct UsedSymbolRefs {
  inner: FxHashSet<SymbolRef>,
}

impl UsedSymbolRefs {
  #[inline]
  pub fn insert(&mut self, symbol_ref: SymbolRef) {
    self.inner.insert(symbol_ref);
  }

  #[inline]
  pub fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }

  #[inline]
  pub fn remove(&mut self, symbol_ref: &SymbolRef) -> bool {
    self.inner.remove(symbol_ref)
  }
}
