use rustc_hash::FxHashSet;

use super::{symbol_ref::SymbolRef, used_symbol_refs_view::UsedSymbolRefsView};

/// The sealed record of inclusion-fixpoint liveness: symbols the inclusion machinery
/// decided are needed as bindings — refs referenced by included statements (in both
/// their original and canonical forms) plus interface-policy retentions (entry exports,
/// CJS bailout, eval-kept imports). Constants that get inlined are deliberately absent
/// (never inserted — their use sites are replaced with the value; constants that must
/// stay bindings, e.g. entry exports, are present), and a normal module's namespace ref
/// is not authoritative here — the generate stage decides namespace retention
/// separately, on `LinkingMetadata::namespace_included`.
///
/// Read-only by construction: produced by [`crate::UsedSymbolRefsBuilder::seal`] once the last
/// writer (the generate stage's unused-runtime-module sweep, after the chunk optimizer's
/// facade-elimination re-run of the inclusion pass) has finished. There is no way to
/// mutate it afterwards.
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
  pub(super) fn new(inner: FxHashSet<SymbolRef>) -> Self {
    Self { inner }
  }

  #[inline]
  pub fn view(&self) -> UsedSymbolRefsView<'_> {
    UsedSymbolRefsView::new(&self.inner)
  }

  #[inline]
  pub fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }
}
