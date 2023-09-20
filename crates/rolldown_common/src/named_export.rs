use oxc::span::Atom;

use super::import_record::ImportRecordId;
use crate::symbol_ref::SymbolRef;

/// This is a representation for statements like
/// - Case A: `export function foo() {}`
/// - Case B: `const foo = 1; export { foo }`
/// - Case C: `const foo = 1; export { foo as foo2 }`
#[derive(Debug)]
pub struct LocalExport {
  pub referenced: SymbolRef,
}

impl From<LocalExport> for LocalOrReExport {
  fn from(value: LocalExport) -> Self {
    Self::Local(value)
  }
}

/// This is a representation for statements like
/// - Case A: `export { foo } from 'foo'`
/// - Case B: `export * as fooNs from 'foo'`
/// - Case C: `export { foo as foo2 } from 'foo'`
#[derive(Debug)]
pub struct ReExport {
  /// For case A, the `imported` is `foo`.
  /// For case B, the `imported` is meaningless.
  /// For case C, the `imported` is `foo`.
  pub imported: Atom,
  /// This will only be `true` for case B.
  pub is_imported_star: bool,
  pub record_id: ImportRecordId,
}

impl From<ReExport> for LocalOrReExport {
  fn from(value: ReExport) -> Self {
    Self::Re(value)
  }
}

#[derive(Debug)]
pub enum LocalOrReExport {
  Local(LocalExport),
  Re(ReExport),
}
