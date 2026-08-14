use rustc_hash::FxHashSet;

use crate::ModuleIdx;

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
  #[inline]
  pub fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }
}

/// The mutable phase of [`UsedSymbolRefs`], held only by the inclusion machinery
/// (the link-stage fixpoint, the chunk optimizer's re-run of it, and the generate
/// stage's unused-runtime-module sweep).
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

  /// Drop every symbol owned by `owner`. Used by the generate stage's runtime
  /// module sweep when the runtime module turns out to be unused after the
  /// entry-level-external walk-back invalidated the link-time reasons for
  /// including it.
  pub fn remove_owned_by(&mut self, owner: ModuleIdx) {
    self.inner.retain(|symbol_ref| symbol_ref.owner != owner);
  }

  pub fn seal(self) -> UsedSymbolRefs {
    UsedSymbolRefs { inner: self.inner }
  }
}
