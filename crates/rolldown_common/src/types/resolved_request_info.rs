use std::{path::Path, sync::Arc};

use arcstr::ArcStr;

use crate::{side_effects::HookSideEffects, ModuleDefFormat, PackageJson};

use super::module_id::stabilize_module_id;

#[derive(Debug)]
pub struct ResolvedId {
  pub id: ArcStr,
  // https://github.com/defunctzombie/package-browser-field-spec/blob/8c4869f6a5cb0de26d208de804ad0a62473f5a03/README.md?plain=1#L62-L77
  pub ignored: bool,
  pub module_def_format: ModuleDefFormat,
  pub is_external: bool,
  pub package_json: Option<Arc<PackageJson>>,
  pub side_effects: Option<HookSideEffects>,
}

impl ResolvedId {
  /// Created a pretty string representation of the path. The path
  /// 1. doesn't guarantee to be unique
  /// 2. relative to the cwd, so it could show stable path across different machines
  pub fn debug_id(&self, cwd: impl AsRef<Path>) -> String {
    if self.id.trim_start().starts_with("data:") {
      return format!("<{}>", self.id);
    }

    let stable = stabilize_module_id(&self.id, cwd.as_ref());
    if self.ignored {
      format!("(ignored) {stable}")
    } else {
      stable
    }
  }
}
