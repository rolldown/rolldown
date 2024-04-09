use index_vec::IndexVec;
use rolldown_common::{ImportRecordId, NormalModuleId, RawImportRecord};
use rolldown_error::BuildError;
use rolldown_oxc_utils::OxcAst;

use crate::types::resolved_request_info::ResolvedRequestInfo;
use crate::types::{ast_symbols::AstSymbols, normal_module_builder::NormalModuleBuilder};

pub struct NormalModuleTaskResult {
  pub module_id: NormalModuleId,
  pub ast_symbol: AstSymbols,
  pub resolved_deps: IndexVec<ImportRecordId, ResolvedRequestInfo>,
  pub raw_import_records: IndexVec<ImportRecordId, RawImportRecord>,
  pub warnings: Vec<BuildError>,
  pub builder: NormalModuleBuilder,
  pub ast: OxcAst,
}
