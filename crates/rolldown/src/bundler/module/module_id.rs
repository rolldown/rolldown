use index_vec::IndexVec;
use rolldown_common::ModuleId;

use super::module::Module;

pub type ModuleVec = IndexVec<ModuleId, Module>;
