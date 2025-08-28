use std::hash::Hash;

use arcstr::ArcStr;

use crate::{ModuleIdx, StmtInfoIdx};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EntryPoint {
  pub name: Option<ArcStr>,
  pub idx: ModuleIdx,
  pub kind: EntryPointKind,
  /// emitted chunk specified filename, used to generate chunk filename
  pub file_name: Option<ArcStr>,
  /// which stmts create this entry point
  pub related_stmt_infos: Vec<(ModuleIdx, StmtInfoIdx)>,
}

#[derive(Debug, Eq, Clone, Copy, PartialOrd, Ord)]
pub enum EntryPointKind {
  UserDefined = 0,
  DynamicImport = 1,
  /// The extra varant [EntryPointKind::EmittedUserDefined] is only used to sort the entry points, it is equal to [EntryPointKind::UserDefined] in terms of functionality.
  EmittedUserDefined = 2,
}

impl Hash for EntryPointKind {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    // Override the hash function to ensure that `EmittedUserDefined` is treated as `UserDefined` for hashing purposes.
    let discriminant = match self {
      EntryPointKind::UserDefined | EntryPointKind::EmittedUserDefined => 0,
      EntryPointKind::DynamicImport => 1,
    };
    discriminant.hash(state);
  }
}

impl PartialEq for EntryPointKind {
  fn eq(&self, other: &Self) -> bool {
    self.is_user_defined() == other.is_user_defined()
  }
}

impl EntryPointKind {
  #[inline]
  pub fn is_user_defined(&self) -> bool {
    matches!(self, EntryPointKind::UserDefined | EntryPointKind::EmittedUserDefined)
  }

  #[inline]
  pub fn is_dynamic_import(&self) -> bool {
    matches!(self, EntryPointKind::DynamicImport)
  }
}
