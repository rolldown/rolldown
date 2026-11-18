use oxc::{
  semantic::{NodeId, ReferenceId},
  span::Span,
};
use oxc_str::CompactStr;

use crate::{MemberExprRefResolution, SymbolRef, type_aliases::MemberExprRefResolutionMap};

/// A single property access in a member expression chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprProp {
  pub name: CompactStr,
  pub span: Span,
  /// Whether this property access uses optional chaining (`?.`).
  pub optional: bool,
}

/// For member expression, e.g. `foo_ns.bar_ns.c`
/// - `object_ref` is the `SymbolRef` that represents `foo_ns`
/// - `props` is `["bar_ns", "c"]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprRef {
  pub object_ref: SymbolRef,
  pub prop_and_span_list: Vec<MemberExprProp>,
  /// Node ID of the whole member expression.
  pub node_id: NodeId,
  /// Span of the whole member expression, used for diagnostics and generated replacement spans.
  pub span: Span,
  pub object_ref_type: MemberExprObjectReferencedType,
  /// The semantic reference ID for the object identifier of this member expression.
  /// Used during symbol renaming to find the scope where the reference occurs,
  /// enabling detection of potential shadowing by nested scope bindings.
  pub reference_id: Option<ReferenceId>,
  /// Whether the recorded expression or a member below it is assigned or deleted
  /// (`ns.a = 1`, `ns.a[k] = 1`). Set from oxc's `MemberWriteTarget` flag or `write_target`.
  pub is_write: bool,
  /// How the recorded expression (`object_ref` plus `prop_and_span_list`) is itself written, if it
  /// is. `ns.a[k] = 1` records only `ns.a`, so `is_write` is true but this is `None`.
  pub write_target: Option<MemberExprWriteKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberExprWriteKind {
  /// `ns.a = 1`, `ns.a++`, `[ns.a] = []`, `for (ns.a of [])`
  Assign,
  /// `delete ns.a`, `delete ns?.a`
  Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberExprObjectReferencedType {
  Named,
  Default,
  Namespace,
}

impl MemberExprRef {
  /// This method is tricky, use it with care.
  /// If this method returns `None`, it means `MemberExprRef` points to nothing and corresponding member expr will be rewritten as `void 0`.
  /// There's no any symbol ref in this `MemberExprRef`.
  /// If this method returns `Some`, it has two possible situations:
  /// 1. The member expr does resolved to a symbol
  /// 2. The member expr doesn't contain module namespace ref and is just a normal member expr.
  pub fn represent_symbol_ref(
    &self,
    resolved_map: &MemberExprRefResolutionMap,
  ) -> Option<SymbolRef> {
    if let Some(resolution) = resolved_map.get(&self.node_id) {
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
    resolved_map.get(&self.node_id)
  }
}
