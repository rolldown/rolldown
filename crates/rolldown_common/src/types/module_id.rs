use crate::NormalModuleId;

use super::external_module_id::ExternalModuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModuleId {
  Normal(NormalModuleId),
  External(ExternalModuleId),
}

impl ModuleId {
  pub fn expect_normal(self) -> NormalModuleId {
    match self {
      Self::Normal(id) => id,
      Self::External(_) => panic!("Expected a normal module id"),
    }
  }

  pub fn as_normal(self) -> Option<NormalModuleId> {
    match self {
      Self::Normal(id) => Some(id),
      Self::External(_) => None,
    }
  }

  pub fn as_external(self) -> Option<ExternalModuleId> {
    match self {
      Self::Normal(_) => None,
      Self::External(id) => Some(id),
    }
  }
}

impl From<NormalModuleId> for ModuleId {
  fn from(v: NormalModuleId) -> Self {
    Self::Normal(v)
  }
}

impl From<ExternalModuleId> for ModuleId {
  fn from(v: ExternalModuleId) -> Self {
    Self::External(v)
  }
}
