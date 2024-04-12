use index_vec::IndexVec;
use rolldown_common::{ExternalModule, ExternalModuleId, NormalModule, NormalModuleId};
use rolldown_utils::fast_drop;

pub type NormalModuleVec = IndexVec<NormalModuleId, NormalModule>;
pub type ExternalModuleVec = IndexVec<ExternalModuleId, ExternalModule>;

#[derive(Debug)]
pub struct ModuleTable {
  pub normal_modules: NormalModuleVec,
  pub external_modules: ExternalModuleVec,
}

impl Drop for ModuleTable {
  fn drop(&mut self) {
    fast_drop(std::mem::take(&mut self.normal_modules));
    fast_drop(std::mem::take(&mut self.external_modules));
  }
}
