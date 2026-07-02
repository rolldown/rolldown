use rustc_hash::FxHashSet;

use super::symbol_ref::SymbolRef;

/// Symbols the inclusion fixpoint decided are needed as bindings: refs referenced
/// by included statements (inserted in both their original and canonical forms)
/// plus interface-policy retentions (entry exports, CJS bailout, eval-kept
/// imports). Constants that get inlined are deliberately absent (never inserted —
/// their use sites are replaced with the value; constants that must stay bindings,
/// e.g. entry exports, are present), and module namespace refs are inserted/removed
/// once more by the generate stage's namespace decision.
///
/// Writers: `include_symbol` during the inclusion pass, the chunk optimizer's
/// facade-elimination re-run of that pass, and the namespace-decision mirror in
/// `finalized_module_namespace_ref_usage`. Nothing else may mutate this.
///
/// Purpose-specific views exist for common questions — prefer them:
/// `LinkingMetadata::namespace_included` for namespace retention,
/// `UsedExternalSymbols` for external bindings, and `RetainedExportSymbols` for
/// a module's retained export interface.
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
