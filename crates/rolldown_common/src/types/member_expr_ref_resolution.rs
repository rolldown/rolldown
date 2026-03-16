use oxc::semantic::ReferenceId;

use crate::SymbolRef;
use crate::types::member_expr_ref::MemberExprProp;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprRefResolution {
  /// `None` means the member expr points to nothing.
  /// Such as
  /// ```js
  /// import * as ns from './foo.js'
  /// console.log(ns.nonExistExport)
  /// ```
  pub resolved: Option<SymbolRef>,
  /// Remaining (unresolved) properties after namespace resolution, e.g. `["bar"]` for
  /// `ns.foo.bar` when `foo` was resolved. Each entry carries the property name, span,
  /// and whether it uses optional chaining (`?.`).
  pub prop_and_related_span_list: Vec<MemberExprProp>,
  /// If you want to include the `resolved` symbol, these are depended symbols that need to be included together to ensure correct runtime behaviors.
  pub depended_refs: Vec<SymbolRef>,
  /// The barrel exports from commonjs is different from es module.
  /// If a symbol is reexport multiple times, we need to store the import record namespace, which
  /// link to the wrapper symbol, unlike es module, we could directly link to exported symbol.
  /// the first element is resolved commonjs export symbol
  /// the second element is whether the exported symbol binds on `exports.default`
  /// This is used to optimize interop between ESM and CommonJS.
  /// e.g.
  /// ```js
  /// import mod from './cjs.js'
  /// console.log(mod.default) // we will not optimize this member expr to generate same interop
  /// code as esbuild
  /// ```
  pub target_commonjs_exported_symbol: Option<(SymbolRef, bool)>,
  /// Propagated from `MemberExprRef::reference_id`. Used during symbol renaming
  /// to find the scope where the member expression's object is referenced,
  /// enabling detection of potential shadowing by nested scope bindings.
  pub reference_id: Option<ReferenceId>,
}
