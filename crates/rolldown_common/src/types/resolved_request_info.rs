use std::sync::Arc;

use crate::{side_effects::HookSideEffects, ModuleDefFormat, PackageJson, ResolvedPath};

#[derive(Debug)]
pub struct ResolvedId {
  pub id: ResolvedPath,
  pub module_def_format: ModuleDefFormat,
  pub is_external: bool,
  pub package_json: Option<Arc<PackageJson>>,
  pub side_effects: Option<HookSideEffects>,
}
