use crate::ModuleIdx;

use super::external_module_idx::ExternalModuleIdx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LegacyModuleIdx {
  Ecma(ModuleIdx),
  External(ExternalModuleIdx),
}

impl LegacyModuleIdx {
  pub fn expect_ecma(self) -> ModuleIdx {
    match self {
      Self::Ecma(id) => id,
      Self::External(_) => panic!("Expected a normal module id"),
    }
  }

  pub fn as_ecma(self) -> Option<ModuleIdx> {
    match self {
      Self::Ecma(id) => Some(id),
      Self::External(_) => None,
    }
  }

  pub fn as_external(self) -> Option<ExternalModuleIdx> {
    match self {
      Self::Ecma(_) => None,
      Self::External(id) => Some(id),
    }
  }
}

impl From<ModuleIdx> for LegacyModuleIdx {
  fn from(v: ModuleIdx) -> Self {
    Self::Ecma(v)
  }
}

impl From<ExternalModuleIdx> for LegacyModuleIdx {
  fn from(v: ExternalModuleIdx) -> Self {
    Self::External(v)
  }
}
