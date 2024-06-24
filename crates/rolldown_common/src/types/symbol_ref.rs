use oxc::semantic::SymbolId;
use oxc::span::CompactStr;

use crate::NormalModuleId;

/// Crossing module ref between symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolRef {
  pub owner: NormalModuleId,
  pub symbol: SymbolId,
}

impl From<(NormalModuleId, SymbolId)> for SymbolRef {
  fn from(value: (NormalModuleId, SymbolId)) -> Self {
    Self { owner: value.0, symbol: value.1 }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolOrMemberExprRef {
  Symbol(SymbolRef),
  MemberExpr(MemberExprRef),
}

impl SymbolOrMemberExprRef {
  /// get the first part of the expression,
  /// e.g. `test.a.b` will return `test`
  /// for identifier, it will return itself
  pub fn symbol_ref(&self) -> &SymbolRef {
    match self {
      SymbolOrMemberExprRef::Symbol(s) => s,
      SymbolOrMemberExprRef::MemberExpr(expr) => &expr.object_ref,
    }
  }
}

/// For member expression, e.g. `foo_ns.bar_ns.c`
/// - `object_ref` is the `SymbolRef` that represents `foo_ns`
/// - `props` is `["bar_ns", "c"]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprRef {
  pub object_ref: SymbolRef,
  pub props: Vec<CompactStr>,
}

impl From<(NormalModuleId, SymbolId)> for SymbolOrMemberExprRef {
  fn from(value: (NormalModuleId, SymbolId)) -> Self {
    Self::Symbol(SymbolRef { owner: value.0, symbol: value.1 })
  }
}

impl From<(NormalModuleId, SymbolId, Vec<CompactStr>)> for SymbolOrMemberExprRef {
  fn from(value: (NormalModuleId, SymbolId, Vec<CompactStr>)) -> Self {
    Self::MemberExpr(MemberExprRef { object_ref: (value.0, value.1).into(), props: value.2 })
  }
}

impl From<SymbolRef> for SymbolOrMemberExprRef {
  fn from(value: SymbolRef) -> Self {
    Self::Symbol(value)
  }
}
