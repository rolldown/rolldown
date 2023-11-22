use crate::ModuleId;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EntryPoint {
  pub name: Option<String>,
  pub module_id: ModuleId,
  pub kind: EntryPointKind,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum EntryPointKind {
  UserSpecified,
  DynamicImport,
}
