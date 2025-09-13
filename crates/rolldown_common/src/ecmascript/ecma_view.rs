use crate::{ConstExportMeta, ImportAttribute, RUNTIME_HELPER_NAMES, StmtInfoIdx};
use arcstr::ArcStr;
use bitflags::bitflags;
use oxc::{
  semantic::SymbolId,
  span::{CompactStr, Span},
};
use oxc_index::IndexVec;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ExportsKind, HmrInfo, ImportRecordIdx, LocalExport, ModuleDefFormat, ModuleId, ModuleIdx,
  NamedImport, ResolvedImportRecord, SourceMutation, StmtInfos, SymbolRef,
  side_effects::DeterminedSideEffects, types::source_mutation::ArcSourceMutation,
};

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct EcmaViewMeta: u8 {
        const Eval = 1;
        const Included = 1 << 1;
        const HasLazyExport = 1 << 2;
        const HasStarExport = 1 << 3;
        const SafelyTreeshakeCommonjs = 1 << 4;
        /// If a module has side effects or has top-level global variable access
        const ExecutionOrderSensitive = 1 << 5;
        /// If the module has top-level empty function, if any module has top level empty function, we need
        /// to apply cross module optimization.
        const TopExportedLevelEmptyFunction = 1 << 6;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ThisExprReplaceKind {
  /// It depends on `context` set by the user. If it's unset, replace it with `undefined`.
  Context,
  Exports,
}

#[inline]
#[expect(clippy::implicit_hasher)]
pub fn generate_replace_this_expr_map(
  set: &FxHashSet<Span>,
  kind: ThisExprReplaceKind,
) -> FxHashMap<Span, ThisExprReplaceKind> {
  set.iter().map(|span| (*span, kind)).collect()
}

impl EcmaViewMeta {
  #[inline]
  pub fn has_eval(&self) -> bool {
    self.contains(Self::Eval)
  }
  #[inline]
  pub fn is_included(&self) -> bool {
    self.contains(Self::Included)
  }
  #[inline]
  pub fn has_lazy_export(&self) -> bool {
    self.contains(Self::HasLazyExport)
  }
  #[inline]
  pub fn has_star_export(&self) -> bool {
    self.contains(Self::HasStarExport)
  }
}

#[derive(Debug, Clone)]
pub struct EcmaView {
  pub dummy_record_set: FxHashSet<Span>,
  pub source: ArcStr,
  pub def_format: ModuleDefFormat,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  pub named_imports: FxIndexMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<CompactStr, LocalExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  /// The key is the `Span` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`
  /// and `CallExpression`(only when the callee is `require`).
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  pub exports_kind: ExportsKind,
  pub default_export_ref: SymbolRef,
  pub sourcemap_chain: Vec<rolldown_sourcemap::SourceMap>,
  // the ids of all modules that statically import this module
  pub importers: FxIndexSet<ModuleId>,
  pub importers_idx: FxIndexSet<ModuleIdx>,
  // the ids of all modules that import this module via dynamic import()
  pub dynamic_importers: FxIndexSet<ModuleId>,
  // the module ids statically imported by this module
  pub imported_ids: FxIndexSet<ModuleId>,
  // the module ids imported by this module via dynamic import()
  pub dynamically_imported_ids: FxIndexSet<ModuleId>,
  pub side_effects: DeterminedSideEffects,
  pub ast_usage: EcmaModuleAstUsage,
  pub self_referenced_class_decl_symbol_ids: FxHashSet<SymbolId>,
  // the range of hashbang in source
  pub hashbang_range: Option<Span>,
  pub directive_range: Vec<Span>,
  pub meta: EcmaViewMeta,
  pub mutations: Vec<ArcSourceMutation>,
  /// `Span` of `new URL('path', import.meta.url)` -> `ImportRecordIdx`
  pub new_url_references: FxHashMap<Span, ImportRecordIdx>,
  pub this_expr_replace_map: FxHashMap<Span, ThisExprReplaceKind>,
  pub depended_runtime_helper: Box<[Vec<StmtInfoIdx>; RUNTIME_HELPER_NAMES.len()]>,

  pub hmr_hot_ref: Option<SymbolRef>,
  pub hmr_info: HmrInfo,
  pub constant_export_map: FxHashMap<SymbolId, ConstExportMeta>,
  pub import_attribute_map: FxHashMap<ImportRecordIdx, ImportAttribute>,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct EcmaModuleAstUsage: u16 {
        const ModuleRef = 1;
        const ExportsRef = 1 << 1;
        /// If the module has `Object.defineProperty(module.exports, "__esModule", { value: true })` or it's variant
        const EsModuleFlag = 1 << 2;
        const AllStaticExportPropertyAccess = 1 << 3;
        /// module.exports = require('mod');
        const IsCjsReexport = 1 << 4;
        const TopLevelAwait = 1 << 5;
        const HmrSelfAccept = 1 << 6;
        const UnknownExportsRead = 1 << 7;
        /// Top-level return statement (only valid in CommonJS)
        const TopLevelReturn = 1 << 8;
        const ModuleOrExports = Self::ModuleRef.bits() | Self::ExportsRef.bits();
    }
}

#[derive(Debug, Default)]
pub struct ImportMetaRolldownAssetReplacer {
  pub asset_filename: ArcStr,
}

impl SourceMutation for ImportMetaRolldownAssetReplacer {
  fn apply(&self, magic_string: &mut string_wizard::MagicString<'_>) {
    magic_string
      .replace_all("import.meta.__ROLLDOWN_ASSET_FILENAME", format!("\"{}\"", self.asset_filename));
  }
}

#[derive(Debug, Default)]
pub struct PrependRenderedImport {
  pub intro: String,
}

impl SourceMutation for PrependRenderedImport {
  fn apply(&self, magic_string: &mut string_wizard::MagicString<'_>) {
    magic_string.prepend(self.intro.clone());
  }
}
