use crate::FilePath;

#[derive(Debug, Clone)]
pub struct PreRenderedChunk {
  // pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<FilePath>,
  pub module_ids: Vec<FilePath>,
  pub exports: Vec<String>,
}
