use arcstr::ArcStr;

use crate::ModuleIdx;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EntryPoint {
  pub name: Option<ArcStr>,
  pub id: ModuleIdx,
  pub kind: EntryPointKind,
  pub file_name: Option<ArcStr>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum EntryPointKind {
  UserDefined,
  DynamicImport,
}

impl EntryPointKind {
  pub fn is_user_defined(&self) -> bool {
    matches!(self, EntryPointKind::UserDefined)
  }
}
