use oxc::span::Atom;

use crate::module_id::ModuleId;

index_vec::define_index_type! {
  pub struct ImportRecordId = u32;
}

#[derive(Debug, PartialEq, Eq)]
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

impl ImportRecord {
  pub fn new(specifier: Atom, kind: ImportKind) -> Self {
    Self {
      module_request: specifier,
      resolved_module: Default::default(),
      is_import_namespace: false,
      kind,
    }
  }
}
