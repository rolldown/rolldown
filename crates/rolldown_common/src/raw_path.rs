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
