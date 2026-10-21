use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_utils::{
  concat_string, dataurl::is_data_url, ecmascript::escape_line_terminators,
  stabilize_id::stabilize_id,
};

use super::module_id::ModuleId;
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

#[derive(Debug, Default, Clone)]
pub struct ResolvedId {
  pub id: ModuleId,
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
      id: ModuleId::default(),
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
  /// 3. has its line terminators escaped, so it can go in a `//` comment
  pub fn debug_id(&self, cwd: impl AsRef<Path>) -> String {
    let debug_id = if is_data_url(&self.id) {
      concat_string!("<", self.id.as_str(), ">")
    } else if let Some(original) = self.id.strip_empty_prefix() {
      concat_string!("(ignored) ", stabilize_id(original, cwd.as_ref()))
    } else {
      stabilize_id(&self.id, cwd.as_ref())
    };
    escape_line_terminators(debug_id)
  }

  pub fn new_external_without_side_effects(id: ArcStr) -> Self {
    Self {
      id: ModuleId::new(id),
      module_def_format: ModuleDefFormat::Unknown,
      external: true.into(),
      normalize_external_id: None,
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: true,
    }
  }
}
