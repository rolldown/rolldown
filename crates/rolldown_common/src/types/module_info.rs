use std::sync::Arc;

#[derive(Debug)]
pub struct ModuleInfo {
  pub code: Option<Arc<str>>,
  pub id: Arc<str>,
}
