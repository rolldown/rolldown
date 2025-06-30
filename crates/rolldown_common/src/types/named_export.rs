use crate::SymbolRef;
use oxc::span::Span;

/// This is a representation for statements like
/// - Case A: `export function foo() {}`
/// - Case B: `const foo = 1; export { foo }`
/// - Case C: `const foo = 1; export { foo as foo2 }`
#[derive(Debug, Clone, Copy)]
pub struct LocalExport {
  pub span: Span,
  pub referenced: SymbolRef,
  /// `true` if the export came from a commonjs module
  /// ```js
  /// exports.foo = 1;
  /// ```
  pub came_from_commonjs: bool,
}
