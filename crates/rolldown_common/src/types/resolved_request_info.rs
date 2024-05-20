use crate::{ModuleType, ResolvedPath, PackageJson};

#[derive(Debug)]
pub struct ResolvedRequestInfo {
  pub path: ResolvedPath,
  pub module_type: ModuleType,
  pub is_external: bool,
  pub package_json: Option<PackageJson>,
}
