use crate::ModuleId;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct EntryPoint {
  pub name: Option<String>,
  pub module_id: ModuleId,
}
