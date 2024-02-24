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
