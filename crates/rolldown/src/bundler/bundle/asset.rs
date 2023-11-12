#[derive(Debug)]
pub struct OutputChunk {
  pub file_name: String,
  pub code: String,
  pub is_entry: bool,
  pub facade_module_id: Option<String>,
}
