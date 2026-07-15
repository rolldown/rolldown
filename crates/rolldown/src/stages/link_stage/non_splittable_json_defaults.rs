use rolldown_common::SymbolRef;
use rustc_hash::FxHashSet;

/// JSON default bindings whose static property reads must remain live object accesses.
///
/// `NormalizeLazyExportsPass` produces this sparse set when a transformed JSON object contains an
/// accessor. Binding is its sole consumer: it canonicalizes these refs, unions graph-wide mutation
/// and escape facts, and disables the `data.key` to named-export rewrite. The set is dropped as soon
/// as binding finishes; the finalizer artifact has a separate, longer lifecycle.
#[derive(Debug, Default)]
pub(super) struct NonSplittableJsonDefaults {
  symbols: FxHashSet<SymbolRef>,
}

impl NonSplittableJsonDefaults {
  pub(super) fn insert(&mut self, symbol_ref: SymbolRef) {
    self.symbols.insert(symbol_ref);
  }

  pub(super) fn iter(&self) -> impl Iterator<Item = SymbolRef> + '_ {
    self.symbols.iter().copied()
  }
}
