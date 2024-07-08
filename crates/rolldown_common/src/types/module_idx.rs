use crate::EcmaModuleIdx;

use super::external_module_idx::ExternalModuleIdx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModuleIdx {
  Ecma(EcmaModuleIdx),
  External(ExternalModuleIdx),
}

impl ModuleIdx {
  pub fn expect_ecma(self) -> EcmaModuleIdx {
    match self {
      Self::Ecma(id) => id,
      Self::External(_) => panic!("Expected a normal module id"),
    }
  }

  pub fn as_ecma(self) -> Option<EcmaModuleIdx> {
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

impl From<EcmaModuleIdx> for ModuleIdx {
  fn from(v: EcmaModuleIdx) -> Self {
    Self::Ecma(v)
  }
}

impl From<ExternalModuleIdx> for ModuleIdx {
  fn from(v: ExternalModuleIdx) -> Self {
    Self::External(v)
  }
}
