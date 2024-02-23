use index_vec::IndexVec;
use rolldown_common::{ImportRecordId, ModuleId, RawImportRecord};
use rolldown_error::BuildError;
use rolldown_oxc::OxcProgram;

use crate::bundler::module::normal_module_builder::NormalModuleBuilder;

use crate::bundler::types::ast_symbols::AstSymbols;
use crate::bundler::types::resolved_request_info::ResolvedRequestInfo;

pub struct NormalModuleTaskResult {
  pub module_id: ModuleId,
  pub ast_symbol: AstSymbols,
  pub resolved_deps: IndexVec<ImportRecordId, ResolvedRequestInfo>,
  pub raw_import_records: IndexVec<ImportRecordId, RawImportRecord>,
  pub warnings: Vec<BuildError>,
  pub builder: NormalModuleBuilder,
  pub ast: OxcProgram,
}
