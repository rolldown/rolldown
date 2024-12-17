use oxc::span::{CompactStr, Span};
use rustc_hash::FxHashMap;

use crate::SymbolRef;

/// For member expression, e.g. `foo_ns.bar_ns.c`
/// - `object_ref` is the `SymbolRef` that represents `foo_ns`
/// - `props` is `["bar_ns", "c"]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprRef {
  pub object_ref: SymbolRef,
  pub props: Vec<CompactStr>,
  /// Span of the whole member expression
  /// FIXME: use `AstNodeId` to identify the MemberExpr instead of `Span`
  /// related discussion: https://github.com/rolldown/rolldown/pull/1818#discussion_r1699374441
  pub span: Span,
}

impl MemberExprRef {
  pub fn new(object_ref: SymbolRef, props: Vec<CompactStr>, span: Span) -> Self {
    Self { object_ref, props, span }
  }

  // #[allow(clippy::manual_map)]: Current code is more readable.
  #[allow(clippy::manual_map)]
  pub fn resolved_symbol_ref(
    &self,
    resolved_map: &FxHashMap<Span, (Option<SymbolRef>, Vec<CompactStr>)>,
  ) -> Option<SymbolRef> {
    if let Some((resolved, _)) = resolved_map.get(&self.span) {
      // This member expression resolve to a ambiguous export if `resolved` equals to `None`, which means it actually resolve to nothing.
      // It would be rewrite to `undefined` in the final code.
      resolved.map(|sym_ref| sym_ref)
    } else {
      Some(self.object_ref)
    }
  }
}
