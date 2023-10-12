use rolldown_common::{ImportRecordId, ModuleId};

use crate::bundler::graph::symbols::SymbolMap;
use crate::bundler::module::module_builder::NormalModuleBuilder;
use crate::bundler::resolve_id::ResolvedRequestInfo;
use crate::BuildError;

pub struct NormalModuleTaskResult {
  pub module_id: ModuleId,
  pub symbol_map: SymbolMap,
  pub resolved_deps: Vec<(ImportRecordId, ResolvedRequestInfo)>,
  pub errors: Vec<BuildError>,
  pub warnings: Vec<BuildError>,
  pub builder: NormalModuleBuilder,
}
