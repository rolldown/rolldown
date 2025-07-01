use oxc::span::CompactStr;

use crate::SymbolRef;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprRefResolution {
  /// `None` means the member expr points to nothing.
  /// Such as
  /// ```js
  /// import * as ns from './foo.js'
  /// console.log(ns.nonExistExport)
  /// ```
  pub resolved: Option<SymbolRef>,
  /// Used to store "foo", "bar" for `ns.foo.bar`.
  pub props: Vec<CompactStr>,
  /// If you want to include the `resolved` symbol, these are depended symbols that need to be included together to ensure correct runtime behaviors.
  pub depended_refs: Vec<SymbolRef>,
  /// The barrel exports from commonjs is different from es module.
  /// If a symbol is reexport multiple times, we need to store the import record namespace, which
  /// link to the wrapper symbol, unlike es module, we could directly link to exported symbol.
  pub is_cjs_symbol: bool,
}
