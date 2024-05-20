use std::sync::Arc;

#[derive(Debug)]
pub struct PackageJson {
  raw: Arc<serde_json::Value>,
}

impl PackageJson {
  pub fn new(raw: impl Into<Arc<serde_json::Value>>) -> Self {
    let raw = raw.into();
    Self { raw }
  }

  pub fn r#type(&self) -> Option<&str> {
    self.raw.get("type").and_then(|v| v.as_str())
  }
}
