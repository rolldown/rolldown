use rolldown_common::{ImportRecordId, ModuleId};
use rolldown_error::BuildError;

use crate::bundler::graph::symbols::AstSymbol;
use crate::bundler::module::normal_module_builder::NormalModuleBuilder;
use crate::bundler::utils::resolve_id::ResolvedRequestInfo;

pub struct NormalModuleTaskResult {
  pub module_id: ModuleId,
  pub ast_symbol: AstSymbol,
  pub resolved_deps: Vec<(ImportRecordId, ResolvedRequestInfo)>,
  pub errors: Vec<BuildError>,
  pub warnings: Vec<BuildError>,
  pub builder: NormalModuleBuilder,
}
