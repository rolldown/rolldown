use crate::{EcmaModule, EcmaModuleId, ExternalModule, ExternalModuleId};
use oxc::index::IndexVec;

pub type IndexEcmaModules = IndexVec<EcmaModuleId, EcmaModule>;
pub type IndexExternalModules = IndexVec<ExternalModuleId, ExternalModule>;

#[derive(Debug)]
pub struct ModuleTable {
  pub ecma_modules: IndexEcmaModules,
  pub external_modules: IndexExternalModules,
}
