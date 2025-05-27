use oxc::span::CompactStr;

use crate::SymbolRef;

/// For member expression, e.g. `foo_ns.bar_ns.c`
/// - `object_ref` is the `SymbolRef` that represents `foo_ns`
/// - `props` is `["bar_ns", "c"]`
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
}
