use oxc::index::IndexVec;
use rolldown_common::{
  ImportRecordIdx, Module, ModuleIdx, RawImportRecord, ResolvedId, SymbolRefDbForModule,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::BuildDiagnostic;

pub struct NormalModuleTaskResult {
  pub module_idx: ModuleIdx,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub warnings: Vec<BuildDiagnostic>,
  pub module: Module,
  pub ecma_related: Option<(EcmaAst, SymbolRefDbForModule)>,
}
