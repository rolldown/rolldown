use crate::{
  dynamic_import_usage::DynamicImportExportsUsage, AstScopes, ImportRecordIdx, Module,
  RawImportRecord, ResolvedId, SymbolRefDbForModule,
};
use oxc_index::IndexVec;
use rolldown_ecmascript::EcmaAst;
use rolldown_error::BuildDiagnostic;
use rustc_hash::FxHashMap;

pub struct NormalModuleTaskResult {
  pub module: Module,
  pub ecma_related: Option<EcmaRelated>,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub warnings: Vec<BuildDiagnostic>,
}

pub struct EcmaRelated {
  pub ast: EcmaAst,
  pub symbols: SymbolRefDbForModule,
  pub ast_scope: AstScopes,
  pub dynamic_import_rec_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
}
