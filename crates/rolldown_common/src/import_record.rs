use std::fmt::Display;

use oxc::span::Atom;

use crate::module_id::ModuleId;

index_vec::define_index_type! {
  pub struct ImportRecordId = u32;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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
  pub module_request: Atom,
  // export * as ns from '...'
  // import * as ns from '...'
  pub is_import_namespace: bool,
  pub kind: ImportKind,
}

impl RawImportRecord {
  pub fn new(specifier: Atom, kind: ImportKind) -> Self {
    Self { module_request: specifier, is_import_namespace: false, kind }
  }

  pub fn into_import_record(self, resolved_module: ModuleId) -> ImportRecord {
    ImportRecord {
      module_request: self.module_request,
      resolved_module,
      is_import_namespace: self.is_import_namespace,
      kind: self.kind,
    }
  }
}

#[derive(Debug)]
pub struct ImportRecord {
  // Module Request
  pub module_request: Atom,
  pub resolved_module: ModuleId,
  // export * as ns from '...'
  // import * as ns from '...'
  pub is_import_namespace: bool,
  pub kind: ImportKind,
}
