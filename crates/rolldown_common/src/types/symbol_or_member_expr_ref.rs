use crate::SymbolRef;

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

impl From<MemberExprRef> for SymbolOrMemberExprRef {
  fn from(value: MemberExprRef) -> Self {
    Self::MemberExpr(value)
  }
}

impl From<SymbolRef> for SymbolOrMemberExprRef {
  fn from(value: SymbolRef) -> Self {
    Self::Symbol(value)
  }
}

#[derive(Debug, Clone, Copy)]
pub enum TaggedSymbolRef {
  /// Some symbols are only used during linking, and will not generate actual symbol in output.
  /// e.g. cjs exports
  /// ```js
  /// exports['foo'] = 1;
  /// ```
  /// Aiming to support cjs tree shaking we need to declare some facade symbol that actual did not
  /// exists in original code, also they should not be
  LinkOnly(SymbolRef),
  Normal(SymbolRef),
}

impl TaggedSymbolRef {
  pub fn inner(&self) -> SymbolRef {
    match self {
      TaggedSymbolRef::LinkOnly(s) => *s,
      TaggedSymbolRef::Normal(s) => *s,
    }
  }
}
