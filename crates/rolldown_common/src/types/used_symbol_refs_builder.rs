use rustc_hash::FxHashSet;

use crate::ModuleIdx;

use super::{
  symbol_ref::SymbolRef, used_symbol_refs::UsedSymbolRefs,
  used_symbol_refs_view::UsedSymbolRefsView,
};

/// The mutable phase of [`UsedSymbolRefs`], held only by the inclusion machinery
/// (the link-stage fixpoint, the chunk optimizer's re-run of it, and the generate
/// stage's unused-runtime-module sweep).
#[derive(Debug, Default)]
pub struct UsedSymbolRefsBuilder {
  inner: FxHashSet<SymbolRef>,
}

impl UsedSymbolRefsBuilder {
  #[inline]
  pub fn view(&self) -> UsedSymbolRefsView<'_> {
    UsedSymbolRefsView::new(&self.inner)
  }

  #[inline]
  pub fn insert(&mut self, symbol_ref: SymbolRef) {
    self.inner.insert(symbol_ref);
  }

  #[inline]
  pub fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }

  /// Drop every symbol owned by `owner`. Used by the generate stage's runtime
  /// module sweep when the runtime module turns out to be unused after the
  /// entry-level-external walk-back invalidated the link-time reasons for
  /// including it.
  pub fn remove_owned_by(&mut self, owner: ModuleIdx) {
    self.inner.retain(|symbol_ref| symbol_ref.owner != owner);
  }

  pub fn seal(self) -> UsedSymbolRefs {
    UsedSymbolRefs::new(self.inner)
  }
}
