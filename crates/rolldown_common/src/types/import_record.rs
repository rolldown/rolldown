use std::{
  fmt::Debug,
  ops::{Deref, DerefMut},
};

use oxc::{
  allocator::Address,
  span::{CompactStr, Span},
};

use crate::{ImportKind, ModuleIdx, ModuleType, StmtInfoIdx, SymbolRef};

oxc_index::define_index_type! {
  pub struct ImportRecordIdx = u32;
}

/// Information about a dynamic import expression, used to track the relationship
/// between an import record and its source location in the AST.
#[derive(Debug, Clone, Copy)]
pub struct DynamicImportExprInfo {
  /// Index of the top-level statement containing the dynamic import expression
  pub stmt_info_idx: StmtInfoIdx,
  /// Address of the `ImportExpression` node in the AST
  pub address: Address,
}

#[derive(Debug, Clone)]
pub struct ImportRecordStateInit {
  pub span: Span,
  /// The importee of this import record is asserted to be this specific module type.
  pub asserted_module_type: Option<ModuleType>,
}

#[derive(Debug, Clone, Copy)]
pub struct ImportRecordStateResolved {
  pub resolved_module: Option<ModuleIdx>,
}

impl ImportRecordStateResolved {
  /// We are extremely sure the import record is resolved when calling this method.
  pub fn into_resolved_module(self) -> ModuleIdx {
    self.resolved_module.expect("ImportRecordStateResolved: module not resolved")
  }
}

bitflags::bitflags! {
  #[derive(Debug, Clone, Copy)]
  pub struct ImportRecordMeta: u16 {
    /// the import is inserted during ast transformation, can't get source slice from the original source file
    const IsUnspannedImport = 1 << 0;
    /// `export * from 'mod'` only
    const IsExportStar = 1 << 1;
    ///  Tell the finalizer to use the runtime "__require()" instead of "require()"
    const CallRuntimeRequire = 1 << 2;
    ///  `require('mod')` is used to load the module only
    const IsRequireUnused = 1 << 3;
    /// if the import record is in a try-catch block
    const InTryCatchBlock = 1 << 4;
    /// Whether it is a pure dynamic import, aka a dynamic import only reference a module without using
    /// its exports e.g.
    /// ```js
    /// import('mod');
    /// import('mod').then(mod => {});
    /// const a = await import('mod'); // the a is never be referenced
    /// ```
    const PureDynamicImport = 1 << 5;
    /// Whether it is a pure dynamic import referenced a side effect free module
    const DeadDynamicImport = 1 << 6;
    /// Whether the import is a top level import
    const IsTopLevel = 1 << 7;
    const JsonModule = 1 << 8;
    /// If a record is a re-export-all from an external module, and that re-export-all chain continues uninterrupted to the entry point,
    /// we can reuse the original re-export-all declaration instead of generating complex interoperability code.
    const EntryLevelExternal = 1 << 9;

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
  /// Information about the dynamic import expression, if this is a dynamic import.
  /// Contains the statement index and AST address for tracking purposes.
  ///
  /// Wrapped in `Box` to reduce the size of `ImportRecord` since this field is only
  /// used for dynamic imports (a minority of import records), keeping the common case small.
  pub dynamic_import_expr_info: Option<Box<DynamicImportExprInfo>>,
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
    dynamic_import_expr_info: Option<Box<DynamicImportExprInfo>>,
  ) -> RawImportRecord {
    RawImportRecord {
      module_request: specifier,
      kind,
      namespace_ref,
      meta: ImportRecordMeta::empty(),
      state: ImportRecordStateInit { span, asserted_module_type: assert_module_type },
      dynamic_import_expr_info,
    }
  }

  pub fn with_meta(mut self, meta: ImportRecordMeta) -> Self {
    self.meta = meta;
    self
  }

  pub fn into_resolved(self, resolved_module: Option<ModuleIdx>) -> ResolvedImportRecord {
    ResolvedImportRecord {
      state: ImportRecordStateResolved { resolved_module },
      module_request: self.module_request,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      meta: self.meta,
      dynamic_import_expr_info: self.dynamic_import_expr_info,
    }
  }
}

pub type ResolvedImportRecord = ImportRecord<ImportRecordStateResolved>;
