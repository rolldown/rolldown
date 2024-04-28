use std::sync::Arc;

use crate::FilePath;

#[derive(Debug)]
pub struct ModuleInfo {
  pub code: Option<Arc<str>>,
  pub id: FilePath,
  pub is_entry: bool,
  pub importers: Vec<FilePath>,
  pub dynamic_importers: Vec<FilePath>,
  pub imported_ids: Vec<FilePath>,
  pub dynamically_imported_ids: Vec<FilePath>,
}
