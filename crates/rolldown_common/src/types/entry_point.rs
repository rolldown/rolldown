use arcstr::ArcStr;

use crate::{ModuleIdx, StmtInfoIdx};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EntryPoint {
  pub name: Option<ArcStr>,
  pub id: ModuleIdx,
  pub kind: EntryPointKind,
  /// emitted chunk specified filename, used to generate chunk filename
  pub file_name: Option<ArcStr>,
  /// which stmts create this entry point
  pub related_stmt_infos: Vec<(ModuleIdx, StmtInfoIdx)>,
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
