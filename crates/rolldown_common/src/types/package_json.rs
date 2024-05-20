use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PackageJson {
  pub raw: Arc<serde_json::Value>,
  /// Realpath to `package.json`. Contains the `package.json` filename.
  pub realpath: PathBuf,
}

impl PackageJson {
  pub fn new(raw: impl Into<Arc<serde_json::Value>>, realpath: PathBuf) -> Self {
    let raw = raw.into();
    Self { raw, realpath }
  }

  pub fn r#type(&self) -> Option<&str> {
    self.raw.get("type").and_then(|v| v.as_str())
  }
}
