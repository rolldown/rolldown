use rustc_hash::FxHashSet;

use super::symbol_ref::SymbolRef;

/// A shared read-only borrow of either phase of used-symbol collection.
///
/// Keeping this view concrete lets readers shared by the mutable and sealed phases compile once
/// without exposing mutation or allowing a builder to satisfy an API that requires the sealed
/// [`crate::UsedSymbolRefs`] artifact.
#[derive(Clone, Copy)]
pub struct UsedSymbolRefsView<'a> {
  inner: &'a FxHashSet<SymbolRef>,
}

impl<'a> UsedSymbolRefsView<'a> {
  #[inline]
  pub(super) fn new(inner: &'a FxHashSet<SymbolRef>) -> Self {
    Self { inner }
  }

  #[inline]
  pub fn contains(self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }
}
