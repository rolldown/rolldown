use oxc::index::IndexVec;
use rolldown_common::{
  ImportRecordId, NormalModule, NormalModuleId, RawImportRecord, ResolvedRequestInfo,
};
use rolldown_error::BuildError;
use rolldown_oxc_utils::OxcAst;

use crate::types::ast_symbols::AstSymbols;

pub struct NormalModuleTaskResult {
  pub module_id: NormalModuleId,
  pub ast_symbol: AstSymbols,
  pub resolved_deps: IndexVec<ImportRecordId, ResolvedRequestInfo>,
  pub raw_import_records: IndexVec<ImportRecordId, RawImportRecord>,
  pub warnings: Vec<BuildError>,
  pub module: NormalModule,
  pub ast: OxcAst,
}
