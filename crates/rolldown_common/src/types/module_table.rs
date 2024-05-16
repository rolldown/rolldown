use std::sync::{Arc, RwLock};

use crate::{ExternalModule, ExternalModuleId, NormalModule, NormalModuleId};
use oxc_index::IndexVec;

pub type NormalModuleVec = IndexVec<NormalModuleId, NormalModule>;
pub type ExternalModuleVec = IndexVec<ExternalModuleId, ExternalModule>;

#[derive(Debug, Default)]
pub struct ModuleTable {
  pub normal_modules: NormalModuleVec,
  pub external_modules: ExternalModuleVec,
}

pub type SharedModuleTable = Arc<RwLock<ModuleTable>>;
