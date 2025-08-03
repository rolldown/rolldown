use std::{
  fmt::Debug,
  ops::{Deref, DerefMut},
};

use oxc::span::{CompactStr, Span};

use crate::{ImportKind, ModuleIdx, ModuleType, StmtInfoIdx, SymbolRef};

oxc_index::define_index_type! {
  pub struct ImportRecordIdx = u32;
}

#[derive(Debug, Clone)]
pub struct ImportRecordStateInit {
  pub span: Span,
  /// The importee of this import record is asserted to be this specific module type.
  pub asserted_module_type: Option<ModuleType>,
}

#[derive(Debug, Clone, Copy)]
pub struct ImportRecordStateResolved {
  pub resolved_module: ModuleIdx,
}

bitflags::bitflags! {
  #[derive(Debug, Clone, Copy)]
  pub struct ImportRecordMeta: u16 {
    /// If it is `import {} from '...'` or `import '...'`
    const IsPlainImport = 1;
    /// the import is inserted during ast transformation, can't get source slice from the original source file
    const IsUnspannedImport = 1 << 1;
    /// `export * from 'mod'` only
    const IsExportStar = 1 << 2;
    ///  Tell the finalizer to use the runtime "__require()" instead of "require()"
    const CallRuntimeRequire = 1 << 3;
    ///  `require('mod')` is used to load the module only
    const IsRequireUnused = 1 << 4;
    /// if the import record is in a try-catch block
    const InTryCatchBlock = 1 << 5;
    /// Whether it is a pure dynamic import, aka a dynamic import only reference a module without using
    /// its exports e.g.
    /// ```js
    /// import('mod');
    /// import('mod').then(mod => {});
    /// const a = await import('mod'); // the a is never be referenced
    /// ```
    const PureDynamicImport = 1 << 6;
    /// Whether it is a pure dynamic import referenced a side effect free module
    const DeadDynamicImport = 1 << 7;
    /// Whether the import is a top level import
    const IsTopLevel = 1 << 8;
    /// Mark namespace of a record could be merged safely
    const SafelyMergeCjsNs = 1 << 9;
    const JsonModule = 1 << 10;

    const TopLevelPureDynamicImport = Self::IsTopLevel.bits() | Self::PureDynamicImport.bits();
  }
}

#[derive(Debug, Clone)]
pub struct ImportRecord<State: Debug + Clone> {
  pub state: State,
  /// `./lib.js` in `import { foo } from './lib.js';`
  pub module_request: CompactStr,
  pub kind: ImportKind,
  /// We will turn `import { foo } from './cjs.js'; console.log(foo);` to `var import_foo = require_cjs(); console.log(importcjs.foo)`;
  /// `namespace_ref` represent the potential `import_foo` in above example. It's useless if we imported n esm module.
  pub namespace_ref: SymbolRef,
  pub meta: ImportRecordMeta,
  pub related_stmt_info_idx: Option<StmtInfoIdx>,
}

impl<State: Debug + Clone> ImportRecord<State> {
  pub fn is_unspanned(&self) -> bool {
    self.meta.contains(ImportRecordMeta::IsUnspannedImport)
  }
}

impl<T: Debug + Clone> Deref for ImportRecord<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.state
  }
}

impl<T: Debug + Clone> DerefMut for ImportRecord<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.state
  }
}

pub type RawImportRecord = ImportRecord<ImportRecordStateInit>;

impl RawImportRecord {
  pub fn new(
    specifier: CompactStr,
    kind: ImportKind,
    namespace_ref: SymbolRef,
    span: Span,
    assert_module_type: Option<ModuleType>,
    related_stmt_info_idx: Option<StmtInfoIdx>,
  ) -> RawImportRecord {
    RawImportRecord {
      module_request: specifier,
      kind,
      namespace_ref,
      meta: ImportRecordMeta::empty(),
      state: ImportRecordStateInit { span, asserted_module_type: assert_module_type },
      related_stmt_info_idx,
    }
  }

  pub fn with_meta(mut self, meta: ImportRecordMeta) -> Self {
    self.meta = meta;
    self
  }

  pub fn into_resolved(self, resolved_module: ModuleIdx) -> ResolvedImportRecord {
    ResolvedImportRecord {
      state: ImportRecordStateResolved { resolved_module },
      module_request: self.module_request,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      meta: self.meta,
      related_stmt_info_idx: self.related_stmt_info_idx,
    }
  }
}

pub type ResolvedImportRecord = ImportRecord<ImportRecordStateResolved>;
