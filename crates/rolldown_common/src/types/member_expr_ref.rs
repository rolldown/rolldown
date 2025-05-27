use oxc::span::{CompactStr, Span};

use crate::{MemberExprRefResolution, SymbolRef, type_aliases::MemberExprRefResolutionMap};

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

  /// This method is tricky, use it with care.
  /// If this method returns `None`, it means`MemberExprRef` points to nothing and corresponding member expr will be rewritten as `void 0`.
  /// There's no any symbol ref in this `MemberExprRef`.
  /// If this method returns `Some`, it has two possible situations:
  /// 1. The member expr does resolved to a symbol
  /// 2. The member expr doesn't contain module namespace ref and is just a normal member expr.
  pub fn represent_symbol_ref(
    &self,
    resolved_map: &MemberExprRefResolutionMap,
  ) -> Option<SymbolRef> {
    if let Some(resolution) = resolved_map.get(&self.span) {
      // If the map does have the resolution, it either produces two results:
      // 1. The member expr points to a exist variable/export, which is `MemberExprRefResolution#resolved`
      // 2. The member expr points to a non-exist variable/export, which means `MemberExprRefResolution#resolved` is `None`.
      resolution.resolved
    } else {
      // If the map doesn't have it, it means this member expr doesn't contain any module namespace ref.
      Some(self.object_ref)
    }
  }

  pub fn resolution<'a>(
    &self,
    resolved_map: &'a MemberExprRefResolutionMap,
  ) -> Option<&'a MemberExprRefResolution> {
    resolved_map.get(&self.span)
  }
}
