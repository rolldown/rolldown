use std::fmt::Display;

use rolldown_rstr::Rstr;

use crate::{ModuleId, SymbolRef};

oxc::index::define_index_type! {
  pub struct ImportRecordId = u32;
}

#[derive(Debug, Clone, Copy)]
pub enum ImportKind {
  Import,
  DynamicImport,
  Require,
}

impl ImportKind {
  pub fn is_static(&self) -> bool {
    matches!(self, Self::Import | Self::Require)
  }
}

impl TryFrom<&str> for ImportKind {
  type Error = String;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "import" => Ok(Self::Import),
      "dynamic-import" => Ok(Self::DynamicImport),
      "require-call" => Ok(Self::Require),
      _ => Err(format!("Invalid import kind: {value:?}")),
    }
  }
}

impl Display for ImportKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Import => write!(f, "import-statement"),
      Self::DynamicImport => write!(f, "dynamic-import"),
      Self::Require => write!(f, "require-call"),
    }
  }
}

#[derive(Debug)]
pub struct RawImportRecord {
  // Module Request
  pub module_request: Rstr,
  pub kind: ImportKind,
  pub namespace_ref: SymbolRef,
  pub contains_import_star: bool,
  pub contains_import_default: bool,
  pub is_plain_import: bool,
}

impl RawImportRecord {
  pub fn new(specifier: Rstr, kind: ImportKind, namespace_ref: SymbolRef) -> Self {
    Self {
      module_request: specifier,
      kind,
      namespace_ref,
      contains_import_default: false,
      contains_import_star: false,
      is_plain_import: false,
    }
  }

  pub fn into_import_record(self, resolved_module: ModuleId) -> ImportRecord {
    ImportRecord {
      module_request: self.module_request,
      resolved_module,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      contains_import_star: self.contains_import_star,
      contains_import_default: self.contains_import_default,
      is_plain_import: self.is_plain_import,
    }
  }
}

#[derive(Debug)]
pub struct ImportRecord {
  // Module Request
  pub module_request: Rstr,
  pub resolved_module: ModuleId,
  pub kind: ImportKind,
  pub namespace_ref: SymbolRef,
  pub contains_import_star: bool,
  pub contains_import_default: bool,
  pub is_plain_import: bool,
}
