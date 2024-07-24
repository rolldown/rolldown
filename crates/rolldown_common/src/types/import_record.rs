use std::fmt::Display;

use rolldown_rstr::Rstr;

use crate::{ModuleIdx, StmtInfoIdx, SymbolRef};

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
  /// See [ImportRecord] for more details.
  pub contains_import_star: bool,
  /// See [ImportRecord] for more details.
  pub contains_import_default: bool,
  /// See [ImportRecord] for more details.
  pub is_plain_import: bool,
  pub stmt_idx: StmtInfoIdx,
}

impl RawImportRecord {
  pub fn new(
    specifier: Rstr,
    kind: ImportKind,
    namespace_ref: SymbolRef,
    stmt_idx: StmtInfoIdx,
  ) -> Self {
    Self {
      module_request: specifier,
      kind,
      namespace_ref,
      contains_import_default: false,
      contains_import_star: false,
      is_plain_import: false,
      stmt_idx,
    }
  }

  pub fn into_import_record(self, resolved_module: ModuleIdx) -> ImportRecord {
    ImportRecord {
      module_request: self.module_request,
      resolved_module,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      contains_import_star: self.contains_import_star,
      contains_import_default: self.contains_import_default,
      is_plain_import: self.is_plain_import,
      stmt_idx: self.stmt_idx,
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
  pub stmt_idx: StmtInfoIdx,
}
