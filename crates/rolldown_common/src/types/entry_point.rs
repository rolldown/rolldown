use arcstr::ArcStr;

use crate::ModuleIdx;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EntryPoint {
  pub name: Option<ArcStr>,
  pub id: ModuleIdx,
  pub kind: EntryPointKind,
  // emitted chunk specified filename, used to generate chunk filename
  pub file_name: Option<ArcStr>,
  // emitted chunk corresponding reference_id, used to `PluginContext#getFileName` to search the emitted chunk name
  pub reference_id: Option<ArcStr>,
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
