use crate::{ModuleType, ResolvedPath};

#[derive(Debug)]
pub struct ResolvedRequestInfo {
  pub path: ResolvedPath,
  pub module_type: ModuleType,
  pub is_external: bool,
}
