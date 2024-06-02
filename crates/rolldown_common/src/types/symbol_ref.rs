use oxc::ast::ast::IdentifierName;
use oxc::semantic::SymbolId;
use oxc::span::{Atom, CompactStr};

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

/// Crossing module ref between symbols
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprRef {
  pub symbol: SymbolRef,
  /// TODO: Only atom should be enough
  /// rest static member expr chain, e.g.
  /// ```js
  /// import {test} frmo './a.js'
  /// test.a.b
  /// ```
  /// `test` stored with `SymbolId`
  /// `a.b` is the rest chain
  pub chains: Vec<CompactStr>,
}

impl MemberExprRef {}

impl From<(NormalModuleId, SymbolId)> for SymbolOrMemberExprRef {
  fn from(value: (NormalModuleId, SymbolId)) -> Self {
    Self::Symbol(SymbolRef { owner: value.0, symbol: value.1 })
  }
}

impl From<(NormalModuleId, SymbolId, Vec<CompactStr>)> for SymbolOrMemberExprRef {
  fn from(value: (NormalModuleId, SymbolId, Vec<CompactStr>)) -> Self {
    Self::MemberExpr(MemberExprRef { symbol: (value.0, value.1).into(), chains: value.2 })
  }
}

impl From<SymbolRef> for SymbolOrMemberExprRef {
  fn from(value: SymbolRef) -> Self {
    Self::Symbol(value)
  }
}
