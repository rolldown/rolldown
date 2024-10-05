use arcstr::ArcStr;
use bitflags::bitflags;
use oxc::{index::IndexVec, span::Span};
use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

use crate::{
  side_effects::DeterminedSideEffects, AstScopes, EcmaAstIdx, ExportsKind, ImportRecordIdx,
  LocalExport, ModuleDefFormat, ModuleId, NamedImport, ResolvedImportRecord, StmtInfos, SymbolRef,
};

#[derive(Debug)]
pub struct EcmaView {
  pub source: ArcStr,
  pub ecma_ast_idx: Option<EcmaAstIdx>,
  pub has_eval: bool,
  pub def_format: ModuleDefFormat,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  pub named_imports: FxHashMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  /// The key is the `Span` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`
  /// and `CallExpression`(only when the callee is `require`).
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  // [[StarExportEntries]] in https://tc39.es/ecma262/#sec-source-text-module-records
  pub star_exports: Vec<ImportRecordIdx>,
  pub exports_kind: ExportsKind,
  pub scope: AstScopes,
  pub default_export_ref: SymbolRef,
  pub sourcemap_chain: Vec<rolldown_sourcemap::SourceMap>,
  pub is_included: bool,
  // the ids of all modules that statically import this module
  pub importers: Vec<ModuleId>,
  // the ids of all modules that import this module via dynamic import()
  pub dynamic_importers: Vec<ModuleId>,
  // the module ids statically imported by this module
  pub imported_ids: Vec<ModuleId>,
  // the module ids imported by this module via dynamic import()
  pub dynamically_imported_ids: Vec<ModuleId>,
  pub side_effects: DeterminedSideEffects,
  pub ast_usage: EcmaModuleAstUsage,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct EcmaModuleAstUsage: u8 {
        const ModuleRef = 1;
        const ExportsRef = 1 << 1;
        const ModuleOrExports = Self::ModuleRef.bits() | Self::ExportsRef.bits();
    }
}
