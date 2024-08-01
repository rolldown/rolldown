use oxc::span::CompactStr;

use crate::SymbolRef;

/// For member expression, e.g. `foo_ns.bar_ns.c`
/// - `object_ref` is the `SymbolRef` that represents `foo_ns`
/// - `props` is `["bar_ns", "c"]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprRef {
  pub object_ref: SymbolRef,
  pub props: Vec<CompactStr>,
}
