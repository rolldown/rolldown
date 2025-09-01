use std::ops::{Deref, DerefMut};

use crate::{Module, ModuleIdx};
use oxc_index::IndexVec;

pub type IndexModules = IndexVec<ModuleIdx, Module>;

#[derive(Debug, Default, Clone)]
pub struct ModuleTable {
  pub modules: IndexModules,
}

impl Deref for ModuleTable {
  type Target = IndexModules;

  fn deref(&self) -> &Self::Target {
    &self.modules
  }
}

impl DerefMut for ModuleTable {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.modules
  }
}
