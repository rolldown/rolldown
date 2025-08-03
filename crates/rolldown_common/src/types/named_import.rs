use std::fmt::Display;

use oxc::span::{CompactStr, Span};

use crate::SymbolRef;

use super::import_record::ImportRecordIdx;

/// This is a representation for statements like
/// - Case A: `import { foo } from 'foo'`
/// - Case B: `import * as fooNs from 'foo'`
/// - Case C: `import { foo as foo2 } from 'foo'`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedImport {
  /// For case A, the `imported` is `foo`.
  /// For case B, the `imported` is meaningless.
  /// For case C, the `imported` is `foo`.
  pub imported: Specifier,
  pub span_imported: Span,
  /// For case A, the `imported_as` is a `SymbolRef` from `foo`.
  /// For case B, the `imported_as` is a `SymbolRef` from `fooNs`.
  /// For case C, the `imported_as` is a `SymbolRef` from `foo2`.
  pub imported_as: SymbolRef,
  pub record_id: ImportRecordIdx,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Specifier {
  Star,
  Literal(CompactStr),
}

impl Specifier {
  pub fn is_star(&self) -> bool {
    matches!(self, Self::Star)
  }
}

impl Display for Specifier {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Star => "*".fmt(f),
      Self::Literal(atom) => atom.as_str().fmt(f),
    }
  }
}

impl From<CompactStr> for Specifier {
  fn from(atom: CompactStr) -> Self {
    Self::Literal(atom)
  }
}

impl From<&str> for Specifier {
  fn from(s: &str) -> Self {
    Self::Literal(CompactStr::new(s))
  }
}
