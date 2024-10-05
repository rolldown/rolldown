use std::{
  fmt::Debug,
  ops::{Deref, DerefMut},
};

use rolldown_rstr::Rstr;

use crate::{ImportKind, ModuleIdx, SymbolRef};

oxc::index::define_index_type! {
  pub struct ImportRecordIdx = u32;
}

#[derive(Debug)]
pub struct ImportRecordStateStart {
  /// Why use start_offset instead of `Span`? Cause, directly pass `Span` will increase the type
  /// size from `40` to `48`(8 bytes alignment). Since the `RawImportRecord` will be created multiple time,
  /// Using this trick could save some memory.
  pub module_request_start: u32,
}

#[derive(Debug)]
pub struct ImportRecordStateResolved {
  pub resolved_module: ModuleIdx,
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

#[derive(Debug)]
pub struct ImportRecord<State: Debug> {
  pub state: State,
  /// `./lib.js` in `import { foo } from './lib.js';`
  pub module_request: Rstr,
  pub kind: ImportKind,
  /// We will turn `import { foo } from './cjs.js'; console.log(foo);` to `var import_foo = require_cjs(); console.log(importcjs.foo)`;
  /// `namespace_ref` represent the potential `import_foo` in above example. It's useless if we imported n esm module.
  pub namespace_ref: SymbolRef,
  pub meta: ImportRecordMeta,
}

impl<T: Debug> Deref for ImportRecord<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.state
  }
}

impl<T: Debug> DerefMut for ImportRecord<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.state
  }
}

pub type RawImportRecord = ImportRecord<ImportRecordStateStart>;

impl RawImportRecord {
  pub fn new(
    specifier: Rstr,
    kind: ImportKind,
    namespace_ref: SymbolRef,
    module_request_start: u32,
  ) -> RawImportRecord {
    RawImportRecord {
      module_request: specifier,
      kind,
      namespace_ref,
      meta: ImportRecordMeta::empty(),
      state: ImportRecordStateStart { module_request_start },
    }
  }

  #[allow(clippy::cast_possible_truncation)]
  pub fn module_request_end(&self) -> u32 {
    self.module_request_start + self.module_request.len() as u32 + 2u32 // +2 for quotes
  }

  pub fn into_resolved(self, resolved_module: ModuleIdx) -> ResolvedImportRecord {
    ResolvedImportRecord {
      state: ImportRecordStateResolved { resolved_module },
      module_request: self.module_request,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      meta: self.meta,
    }
  }
}

pub type ResolvedImportRecord = ImportRecord<ImportRecordStateResolved>;
