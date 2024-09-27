use crate::{ExternalModule, ExternalModuleIdx, Module, ModuleIdx};
use oxc::index::IndexVec;

pub type IndexModules = IndexVec<ModuleIdx, Module>;
pub type IndexExternalModules = IndexVec<ExternalModuleIdx, ExternalModule>;

#[derive(Debug, Default)]
pub struct ModuleTable {
  pub modules: IndexModules,
}
