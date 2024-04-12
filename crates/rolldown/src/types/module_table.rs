use index_vec::IndexVec;
use rolldown_common::{ExternalModule, ExternalModuleId, NormalModule, NormalModuleId};

pub type NormalModuleVec = IndexVec<NormalModuleId, NormalModule>;
pub type ExternalModuleVec = IndexVec<ExternalModuleId, ExternalModule>;

#[derive(Debug)]
pub struct ModuleTable {
  pub normal_modules: NormalModuleVec,
  pub external_modules: ExternalModuleVec,
}

impl Drop for ModuleTable {
  fn drop(&mut self) {
    use rayon::prelude::*;
    std::mem::take(&mut self.normal_modules).into_iter().par_bridge().for_each(std::mem::drop);
    std::mem::take(&mut self.external_modules).into_iter().par_bridge().for_each(std::mem::drop);
  }
}
