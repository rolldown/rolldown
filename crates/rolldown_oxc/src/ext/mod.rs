use oxc::ast::ast;
use oxc::semantic::SymbolId;
pub trait BindingIdentifierExt {
  fn expect_symbol_id(&self) -> SymbolId;
}

impl BindingIdentifierExt for ast::BindingIdentifier {
  #[inline]
  fn expect_symbol_id(&self) -> SymbolId {
    self
      .symbol_id
      .get()
      .unwrap_or_else(|| panic!("fail get symbol id from {:?}", self))
  }
}
