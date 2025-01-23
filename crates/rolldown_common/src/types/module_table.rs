use crate::{ExternalModule, ExternalModuleIdx, Module, ModuleIdx};
use oxc_index::IndexVec;

pub type IndexModules = IndexVec<ModuleIdx, Module>;
pub type IndexExternalModules = IndexVec<ExternalModuleIdx, ExternalModule>;

#[derive(Debug, Default, Clone)]
pub struct ModuleTable {
  pub modules: IndexModules,
}
