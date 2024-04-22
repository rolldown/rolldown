use std::sync::Arc;

use crate::FilePath;

#[derive(Debug)]
pub struct ModuleInfo {
  pub code: Option<Arc<str>>,
  pub id: FilePath,
}
