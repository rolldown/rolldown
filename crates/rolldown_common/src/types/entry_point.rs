use crate::NormalModuleId;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EntryPoint {
  pub name: Option<String>,
  pub id: NormalModuleId,
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
