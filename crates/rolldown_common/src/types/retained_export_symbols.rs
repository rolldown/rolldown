use rustc_hash::FxHashSet;

use super::symbol_ref::SymbolRef;

/// The retained export interface: symbols of resolved exports that survived
/// tree-shaking (either genuinely referenced or kept by interface policy —
/// entry exports, CJS bailout, eval).
///
/// Membership is tracked per ref form, mirroring `used_symbol_refs`: for every
/// module's `resolved_exports` entry, the recorded `symbol_ref` is present iff
/// that exact ref was marked used, and its canonical form iff the canonical
/// value was marked used (possibly through another alias). The two answer
/// different questions, so query with the same form the call site previously
/// used against `used_symbol_refs`. Projected by the generate stage right
/// after the module-namespace decision (`finalized_module_namespace_ref_usage`)
/// and read-only afterwards.
#[derive(Debug, Default)]
pub struct RetainedExportSymbols {
  inner: FxHashSet<SymbolRef>,
}

impl RetainedExportSymbols {
  #[inline]
  pub fn insert(&mut self, symbol_ref: SymbolRef) {
    self.inner.insert(symbol_ref);
  }

  #[inline]
  pub fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }
}
