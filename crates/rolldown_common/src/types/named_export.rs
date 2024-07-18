use crate::SymbolRef;
use oxc::span::Span;

/// This is a representation for statements like
/// - Case A: `export function foo() {}`
/// - Case B: `const foo = 1; export { foo }`
/// - Case C: `const foo = 1; export { foo as foo2 }`
#[derive(Debug)]
pub struct LocalExport {
  pub span: Span,
  pub referenced: SymbolRef,
}
