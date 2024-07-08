use oxc::index::IndexVec;
use rolldown_common::{
  EcmaModule, EcmaModuleIdx, ImportRecordIdx, RawImportRecord, ResolvedRequestInfo,
};
use rolldown_error::BuildError;
use rolldown_oxc_utils::OxcAst;

use crate::types::ast_symbols::AstSymbols;

pub struct NormalModuleTaskResult {
  pub module_id: EcmaModuleIdx,
  pub ast_symbol: AstSymbols,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedRequestInfo>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub warnings: Vec<BuildError>,
  pub module: EcmaModule,
  pub ast: OxcAst,
}
