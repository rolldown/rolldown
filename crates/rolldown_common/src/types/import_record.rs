use std::fmt::Display;

use rolldown_rstr::Rstr;

use crate::{ModuleIdx, SymbolRef};

oxc::index::define_index_type! {
  pub struct ImportRecordIdx = u32;
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

/// See [ImportRecord] for more details.
#[derive(Debug)]
pub struct RawImportRecord {
  // Module Request
  pub module_request: Rstr,
  pub kind: ImportKind,
  /// See [ImportRecord] for more details.
  pub namespace_ref: SymbolRef,
  pub module_request_start: u32,
  pub import_record_meta: ImportRecordMeta,
}

bitflags::bitflags! {
  #[derive(Debug)]
  pub struct ImportRecordMeta: u8 {
    /// See [ImportRecord] for more details.
    const CONTAINS_IMPORT_STAR = 1;
    /// See [ImportRecord] for more details.
    const CONTAINS_IMPORT_DEFAULT = 1 << 1;
    /// See [ImportRecord] for more details.
    const IS_PLAIN_IMPORT = 1 << 2;
  }
}

impl RawImportRecord {
  pub fn new(
    specifier: Rstr,
    kind: ImportKind,
    namespace_ref: SymbolRef,
    module_request_start: u32,
  ) -> Self {
    Self {
      module_request: specifier,
      kind,
      namespace_ref,
      module_request_start,
      import_record_meta: ImportRecordMeta::empty(),
    }
  }

  pub fn into_import_record(self, resolved_module: ModuleIdx) -> ImportRecord {
    ImportRecord {
      module_request: self.module_request,
      resolved_module,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      contains_import_star: self
        .import_record_meta
        .contains(ImportRecordMeta::CONTAINS_IMPORT_STAR),
      contains_import_default: self
        .import_record_meta
        .contains(ImportRecordMeta::CONTAINS_IMPORT_DEFAULT),
      is_plain_import: self.import_record_meta.contains(ImportRecordMeta::IS_PLAIN_IMPORT),
    }
  }
}

#[derive(Debug)]
pub struct ImportRecord {
  // Module Request
  pub module_request: Rstr,
  pub resolved_module: ModuleIdx,
  pub kind: ImportKind,
  /// We will turn `import { foo } from './cjs.js'; console.log(foo);` to `var import_foo = require_cjs(); console.log(importcjs.foo)`;
  /// `namespace_ref` represent the potential `import_foo` in above example. It's useless if we imported n esm module.
  pub namespace_ref: SymbolRef,
  /// If it is `import * as ns from '...'` or `export * as ns from '...'`
  pub contains_import_star: bool,
  /// If it is `import def from '...'`, `import { default as def }`, `export { default as def }` or `export { default } from '...'`
  pub contains_import_default: bool,
  /// If it is `import {} from '...'` or `import '...'`
  pub is_plain_import: bool,
}
