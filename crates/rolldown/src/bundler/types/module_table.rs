use index_vec::IndexVec;
use rolldown_common::{ExternalModule, ExternalModuleId, NormalModuleId};

use crate::bundler::module::NormalModule;

pub type NormalModuleVec = IndexVec<NormalModuleId, NormalModule>;
pub type ExternalModuleVec = IndexVec<ExternalModuleId, ExternalModule>;

#[derive(Debug)]
pub struct ModuleTable {
  pub normal_modules: NormalModuleVec,
  pub external_modules: ExternalModuleVec,
}
