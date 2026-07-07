use rustc_hash::FxHashSet;

use super::symbol_ref::SymbolRef;

/// Symbols owned by external modules that are used by included code.
///
/// Keys are refs whose `owner` is an external module: an external module's
/// `namespace_ref`, or the per-name facade symbols created by the external
/// import binding merger. Importer-local bindings that link to them are not
/// recorded — query with the canonical (linked) ref.
///
/// Written only by the inclusion pass (`include_symbol`); read by output
/// formats and chunk-level deconflicting.
#[derive(Debug, Default)]
pub struct UsedExternalSymbols {
  inner: FxHashSet<SymbolRef>,
}

impl UsedExternalSymbols {
  #[inline]
  pub fn insert(&mut self, symbol_ref: SymbolRef) {
    self.inner.insert(symbol_ref);
  }

  #[inline]
  pub fn contains(&self, symbol_ref: &SymbolRef) -> bool {
    self.inner.contains(symbol_ref)
  }
}
