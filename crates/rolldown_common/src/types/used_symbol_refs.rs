use rustc_hash::FxHashSet;

use super::symbol_ref::SymbolRef;

/// The sealed record of inclusion-fixpoint liveness: symbols the inclusion machinery
/// decided are needed as bindings — refs referenced by included statements (in both
/// their original and canonical forms) plus interface-policy retentions (entry exports,
/// CJS bailout, eval-kept imports). Constants that get inlined are deliberately absent
/// (never inserted — their use sites are replaced with the value; constants that must
/// stay bindings, e.g. entry exports, are present), and a normal module's namespace ref
/// is not authoritative here — the generate stage decides namespace retention
/// separately, on `LinkingMetadata::namespace_included`.
///
/// Read-only by construction: produced by [`UsedSymbolRefsBuilder::seal`] once the last
/// writer (the chunk optimizer's facade-elimination re-run of the inclusion pass) has
/// finished. There is no way to mutate it afterwards.
///
/// Purpose-specific views exist for common questions — prefer them:
/// `LinkingMetadata::namespace_included` for namespace retention,
/// `UsedExternalSymbols` for external bindings, and `RetainedExportSymbols` for
/// a module's retained export interface.
#[derive(Debug)]
pub struct UsedSymbolRefs {
  inner: FxHashSet<SymbolRef>,
}

impl UsedSymbolRefs {
  #[inline]
  pub fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }
}

/// The mutable phase of [`UsedSymbolRefs`], held only by the inclusion machinery
/// (the link-stage fixpoint and the chunk optimizer's re-run of it).
#[derive(Debug, Default)]
pub struct UsedSymbolRefsBuilder {
  inner: FxHashSet<SymbolRef>,
}

impl UsedSymbolRefsBuilder {
  #[inline]
  pub fn insert(&mut self, symbol_ref: SymbolRef) {
    self.inner.insert(symbol_ref);
  }

  #[inline]
  pub fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }

  pub fn seal(self) -> UsedSymbolRefs {
    UsedSymbolRefs { inner: self.inner }
  }
}
