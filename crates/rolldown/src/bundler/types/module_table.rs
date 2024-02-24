use index_vec::IndexVec;
use rolldown_common::{ExternalModuleId, NormalModuleId};

use crate::bundler::module::{external_module::ExternalModule, NormalModule};

pub type NormalModuleVec = IndexVec<NormalModuleId, NormalModule>;
pub type ExternalModuleVec = IndexVec<ExternalModuleId, ExternalModule>;

#[derive(Debug)]
pub struct ModuleTable {
  pub normal_modules: NormalModuleVec,
  pub external_modules: ExternalModuleVec,
}
