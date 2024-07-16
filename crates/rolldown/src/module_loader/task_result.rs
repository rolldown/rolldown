use oxc::index::IndexVec;
use rolldown_common::{
  EcmaModule, ImportRecordIdx, ModuleIdx, RawImportRecord, ResolvedRequestInfo,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::BuildDiagnostic;

use crate::types::ast_symbols::AstSymbols;

pub struct NormalModuleTaskResult {
  pub module_id: ModuleIdx,
  pub ast_symbol: AstSymbols,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedRequestInfo>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub warnings: Vec<BuildDiagnostic>,
  pub module: EcmaModule,
  pub ast: EcmaAst,
}
