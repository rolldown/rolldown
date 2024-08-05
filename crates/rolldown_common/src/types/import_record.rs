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
  /// Why use start_offset instead of `Span`? Cause, directly pass `Span` will increase the type
  /// size from `40` to `48`(8 bytes alignment). Since the `RawImportRecord` will be created multiple time,
  /// Using this trick could save some memory.
  pub module_request_start: u32,
  pub meta: ImportRecordMeta,
}

bitflags::bitflags! {
  #[derive(Debug)]
  pub struct ImportRecordMeta: u8 {
    /// If it is `import * as ns from '...'` or `export * as ns from '...'`
    const CONTAINS_IMPORT_STAR = 1;
    /// If it is `import def from '...'`, `import { default as def }`, `export { default as def }` or `export { default } from '...'`
    const CONTAINS_IMPORT_DEFAULT = 1 << 1;
    /// If it is `import {} from '...'` or `import '...'`
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
      meta: ImportRecordMeta::empty(),
    }
  }

  #[allow(clippy::cast_possible_truncation)]
  pub fn module_request_end(&self) -> u32 {
    self.module_request_start + self.module_request.len() as u32 + 2u32 // +2 for quotes
  }

  pub fn into_import_record(self, resolved_module: ModuleIdx) -> ImportRecord {
    ImportRecord {
      module_request: self.module_request,
      resolved_module,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      meta: self.meta,
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
  pub meta: ImportRecordMeta,
}
