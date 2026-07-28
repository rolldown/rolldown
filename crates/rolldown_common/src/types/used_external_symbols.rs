use rustc_hash::{FxHashMap, FxHashSet};

use super::symbol_ref::SymbolRef;

/// How included code observes an external module as an ES module.
///
/// `require("external")` hands back the raw CommonJS exports object, so non-ESM output formats
/// must run it through `__toESM` whenever the bundle reads it as a namespace object
/// (`import * as ns from 'external'`) or through `ns.default`. A named-only import
/// (`import { foo }`) reads the CommonJS object directly and must *not* be wrapped.
///
/// The two flags keep apart importers Node treats as ESM (rendered as `__toESM(mod, 1)`) from the
/// rest; one external can be observed both ways within a single chunk, which is rendered as two
/// separate bindings.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExternalInteropUse {
  pub node_esm: bool,
  pub non_node_esm: bool,
}

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
  /// Subset of `inner`, restricted to external `namespace_ref`s, recording which of them need the
  /// `__toESM` interop wrapper and how. Tracked separately from the statically-written imports a
  /// chunk's own modules carry, because the module that wrote the import may itself have been
  /// tree-shaken away while the reference to it survived (issue #10069).
  interop_uses: FxHashMap<SymbolRef, ExternalInteropUse>,
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

  /// Record that `namespace_ref` (an external module's canonical namespace ref) is observed as an
  /// ES module by an importer that Node does (`node_esm`) or does not treat as ESM.
  #[inline]
  pub fn note_interop_use(&mut self, namespace_ref: SymbolRef, node_esm: bool) {
    let use_ = self.interop_uses.entry(namespace_ref).or_default();
    if node_esm {
      use_.node_esm = true;
    } else {
      use_.non_node_esm = true;
    }
  }

  #[inline]
  pub fn interop_use(&self, namespace_ref: &SymbolRef) -> Option<ExternalInteropUse> {
    self.interop_uses.get(namespace_ref).copied()
  }

  #[inline]
  pub fn has_interop_use(&self) -> bool {
    !self.interop_uses.is_empty()
  }
}
