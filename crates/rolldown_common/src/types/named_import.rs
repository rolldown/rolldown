use std::fmt::Display;

use oxc::{ast::ast::ModuleExportName, span::Span};
use rolldown_rstr::Rstr;
use rolldown_utils::ecmascript::is_validate_identifier_name;

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
  Literal(ImportOrExportName),
}

impl From<ImportOrExportName> for Specifier {
  fn from(name: ImportOrExportName) -> Self {
    Self::Literal(name)
  }
}

impl Specifier {
  pub fn is_star(&self) -> bool {
    matches!(self, Self::Star)
  }

  pub fn is_default(&self) -> bool {
    matches!(self, Self::Literal(atom) if atom.as_str() == "default")
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ImportOrExportName {
  Identifier(Rstr),
  String(Rstr),
}
/// the Custom impl `PartialOrd` is required, because we don't want to
/// affect the original order by the enum `Tag`
#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for ImportOrExportName {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.as_str().partial_cmp(other.as_str())
  }
}
/// needs to impl `Ord` either, caused by https://rust-lang.github.io/rust-clippy/master/index.html#derive_ord_xor_partial_ord
impl Ord for ImportOrExportName {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.partial_cmp(other).unwrap()
  }
}

impl ImportOrExportName {
  pub fn as_str(&self) -> &str {
    match self {
      Self::Identifier(rstr) | Self::String(rstr) => rstr.as_str(),
    }
  }

  pub fn as_rstr(&self) -> &Rstr {
    match self {
      Self::Identifier(value) | Self::String(value) => value,
    }
  }

  pub fn cmp_to_str(&self, other: &str) -> bool {
    match self {
      Self::Identifier(str) if other.len() == str.len() => str.as_str() == other,
      Self::String(rstr) if rstr.len() == other.len() + 2 => {
        let str = rstr.as_str();
        str.starts_with('"') && str.ends_with('"') && &str[1..str.len() - 1] == other
      }
      _ => false,
    }
  }
}

impl AsRef<str> for ImportOrExportName {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl From<(Rstr, bool)> for ImportOrExportName {
  fn from((rstr, is_valid_ident): (Rstr, bool)) -> Self {
    if is_valid_ident {
      Self::Identifier(rstr)
    } else {
      Self::String(rstr)
    }
  }
}

impl From<Rstr> for ImportOrExportName {
  fn from(rstr: Rstr) -> Self {
    if is_validate_identifier_name(rstr.as_str()) {
      Self::Identifier(rstr)
    } else {
      Self::String(rstr)
    }
  }
}

impl<'a, 'ast: 'a> From<&'a ModuleExportName<'ast>> for ImportOrExportName {
  fn from(name: &'a ModuleExportName) -> Self {
    match name {
      ModuleExportName::IdentifierName(value) => Self::Identifier(value.name.as_str().into()),
      ModuleExportName::StringLiteral(value) => Self::String(value.value.as_str().into()),
      ModuleExportName::IdentifierReference(_) => unreachable!(),
    }
  }
}

impl Display for ImportOrExportName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Identifier(rstr) => rstr.as_str().fmt(f),
      Self::String(rstr) => format!("\"{}\"", rstr.as_str()).fmt(f),
    }
  }
}
