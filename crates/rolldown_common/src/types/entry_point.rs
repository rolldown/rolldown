use std::hash::Hash;

use arcstr::ArcStr;
use oxc::allocator::Address;

use crate::{ImportRecordIdx, ModuleIdx, StmtInfoIdx};

oxc_index::define_index_type! { pub struct EntryIdx = u32; }

#[derive(Debug, Clone)]
pub struct EntryPoint {
  pub name: Option<ArcStr>,
  pub module_idx: ModuleIdx,
  pub entry_index: EntryIdx,
  pub kind: EntryPointKind,
  /// emitted chunk specified filename, used to generate chunk filename
  pub file_name: Option<ArcStr>,
  /// which stmts create this entry point
  pub related_stmt_infos: Vec<(ModuleIdx, StmtInfoIdx, Address, ImportRecordIdx)>,
}

impl Hash for EntryPoint {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.name.hash(state);
    self.module_idx.hash(state);
    self.kind.hash(state);
    self.file_name.hash(state);
    self.related_stmt_infos.hash(state);
  }
}

impl PartialEq for EntryPoint {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name
      && self.module_idx == other.module_idx
      && self.kind == other.kind
      && self.file_name == other.file_name
      && self.related_stmt_infos == other.related_stmt_infos
  }
}

impl Eq for EntryPoint {}

#[derive(Debug, Eq, Clone, Copy, PartialOrd, Ord)]
pub enum EntryPointKind {
  UserDefined = 0,
  DynamicImport = 1,
  /// The extra variant [EntryPointKind::EmittedUserDefined] is only used to sort the entry points, it is equal to [EntryPointKind::UserDefined] in terms of functionality.
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
  pub fn is_emitted_user_defined(&self) -> bool {
    matches!(self, EntryPointKind::EmittedUserDefined)
  }

  #[inline]
  pub fn is_dynamic_import(&self) -> bool {
    matches!(self, EntryPointKind::DynamicImport)
  }
}
