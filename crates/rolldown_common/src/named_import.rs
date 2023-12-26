use oxc::span::{Atom, Span};

use super::import_record::ImportRecordId;
use crate::symbol_ref::SymbolRef;

/// This is a representation for statements like
/// - Case A: `import { foo } from 'foo'`
/// - Case B: `import * as fooNs from 'foo'`
/// - Case C: `import { foo as foo2 } from 'foo'`
#[derive(Debug, Clone)]
pub struct NamedImport {
  /// For case A, the `imported` is `foo`.
  /// For case B, the `imported` is meaningless.
  /// For case C, the `imported` is `foo`.
  pub imported: Specifier,
  /// For case A, the `imported_as` is a `SymbolRef` from `foo`.
  /// For case B, the `imported_as` is a `SymbolRef` from `fooNs`.
  /// For case C, the `imported_as` is a `SymbolRef` from `foo2`.
  pub imported_as: SymbolRef,
  pub record_id: ImportRecordId,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Specifier {
  Star,
  Literal(SpecifierLiteral),
}

impl Specifier {
  pub fn is_star(&self) -> bool {
    matches!(self, Self::Star)
  }
}

impl From<SpecifierLiteral> for Specifier {
  fn from(value: SpecifierLiteral) -> Self {
    Self::Literal(value)
  }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SpecifierLiteral {
  pub span: Span,
  pub name: Atom,
}
