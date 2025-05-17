use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_utils::stabilize_id::stabilize_id;

use crate::{ModuleDefFormat, PackageJson, side_effects::HookSideEffects};

#[derive(Debug, Clone, Copy)]
pub enum ResolvedExternal {
  Bool(bool),
  Absolute,
  Relative,
}

impl Default for ResolvedExternal {
  fn default() -> Self {
    ResolvedExternal::Bool(false)
  }
}

impl ResolvedExternal {
  pub fn is_external(&self) -> bool {
    match self {
      ResolvedExternal::Bool(b) => *b,
      _ => true,
    }
  }
}

impl From<bool> for ResolvedExternal {
  fn from(b: bool) -> Self {
    ResolvedExternal::Bool(b)
  }
}

#[derive(Debug, Clone)]
pub struct ResolvedId {
  pub id: ArcStr,
  // https://github.com/defunctzombie/package-browser-field-spec/blob/8c4869f6a5cb0de26d208de804ad0a62473f5a03/README.md?plain=1#L62-L77
  pub ignored: bool,
  pub module_def_format: ModuleDefFormat,
  pub external: ResolvedExternal,
  // If the js side is return object, the relative id is finally id, else it will be converted to an absolute id
  pub normalize_external_id: Option<bool>,
  pub package_json: Option<Arc<PackageJson>>,
  pub side_effects: Option<HookSideEffects>,
  pub is_external_without_side_effects: bool,
}

impl ResolvedId {
  /// Create a dummy ResolvedId, which is not exists in the file system
  /// note: A dummy `ResolvedId` usually used with `DUMMY_MODULE_IDX`
  pub fn make_dummy() -> Self {
    Self {
      id: arcstr::literal!(""),
      ignored: false,
      module_def_format: ModuleDefFormat::Unknown,
      external: false.into(),
      normalize_external_id: None,
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: false,
    }
  }

  /// Created a pretty string representation of the path. The path
  /// 1. doesn't guarantee to be unique
  /// 2. relative to the cwd, so it could show stable path across different machines
  pub fn debug_id(&self, cwd: impl AsRef<Path>) -> String {
    if self.id.trim_start().starts_with("data:") {
      return format!("<{}>", self.id);
    }

    let stable = stabilize_id(&self.id, cwd.as_ref());
    if self.ignored { format!("(ignored) {stable}") } else { stable }
  }

  pub fn new_external_without_side_effects(id: ArcStr) -> Self {
    Self {
      id,
      ignored: false,
      module_def_format: ModuleDefFormat::Unknown,
      external: true.into(),
      normalize_external_id: None,
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: true,
    }
  }
}
