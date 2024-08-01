use oxc::{semantic::SymbolId, span::CompactStr};

use crate::{ModuleIdx, SymbolRef};

use super::member_expr_ref::MemberExprRef;

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

impl From<(ModuleIdx, SymbolId)> for SymbolOrMemberExprRef {
  fn from(value: (ModuleIdx, SymbolId)) -> Self {
    Self::Symbol(SymbolRef { owner: value.0, symbol: value.1 })
  }
}

impl From<(ModuleIdx, SymbolId, Vec<CompactStr>)> for SymbolOrMemberExprRef {
  fn from(value: (ModuleIdx, SymbolId, Vec<CompactStr>)) -> Self {
    Self::MemberExpr(MemberExprRef { object_ref: (value.0, value.1).into(), props: value.2 })
  }
}

impl From<SymbolRef> for SymbolOrMemberExprRef {
  fn from(value: SymbolRef) -> Self {
    Self::Symbol(value)
  }
}
