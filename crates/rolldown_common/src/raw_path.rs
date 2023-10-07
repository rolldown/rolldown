use std::path::Path;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct RawPath(String);

impl std::ops::Deref for RawPath {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<String> for RawPath {
  fn from(value: String) -> Self {
    Self(value)
  }
}

impl RawPath {
  pub fn file_prefix(&self) -> Option<String> {
    // TODO rust nightly has Path::file_prefix to do this
    Path::new(self.as_str())
      .file_name()
      .map(|p| p.to_string_lossy().split('.').next().unwrap().to_string())
  }
}
