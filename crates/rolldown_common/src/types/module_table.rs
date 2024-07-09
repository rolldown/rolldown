use crate::{EcmaModule, EcmaModuleIdx, ExternalModule, ExternalModuleIdx};
use oxc::index::IndexVec;

pub type IndexEcmaModules = IndexVec<EcmaModuleIdx, EcmaModule>;
pub type IndexExternalModules = IndexVec<ExternalModuleIdx, ExternalModule>;

#[derive(Debug)]
pub struct ModuleTable {
  pub ecma_modules: IndexEcmaModules,
  pub external_modules: IndexExternalModules,
}
