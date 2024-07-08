use crate::EcmaModuleId;

use super::external_module_id::ExternalModuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModuleId {
  Normal(EcmaModuleId),
  External(ExternalModuleId),
}

impl ModuleId {
  pub fn expect_ecma(self) -> EcmaModuleId {
    match self {
      Self::Normal(id) => id,
      Self::External(_) => panic!("Expected a normal module id"),
    }
  }

  pub fn as_ecma(self) -> Option<EcmaModuleId> {
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

impl From<EcmaModuleId> for ModuleId {
  fn from(v: EcmaModuleId) -> Self {
    Self::Normal(v)
  }
}

impl From<ExternalModuleId> for ModuleId {
  fn from(v: ExternalModuleId) -> Self {
    Self::External(v)
  }
}
