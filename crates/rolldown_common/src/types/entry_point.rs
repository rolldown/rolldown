use arcstr::ArcStr;

use crate::ModuleIdx;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EntryPoint {
  pub name: Option<ArcStr>,
  pub id: ModuleIdx,
  pub kind: EntryPointKind,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum EntryPointKind {
  UserDefined,
  DynamicImport,
}

impl EntryPointKind {
  pub fn is_user_defined(&self) -> bool {
    matches!(self, EntryPointKind::UserDefined)
  }
}
