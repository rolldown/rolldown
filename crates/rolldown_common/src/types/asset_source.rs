#[derive(Debug, Clone)]
pub enum AssetSource {
  String(String),
  Buffer(Vec<u8>),
}

impl AssetSource {
  pub fn as_bytes(&self) -> &[u8] {
    match self {
      Self::String(value) => value.as_bytes(),
      Self::Buffer(value) => value.as_ref(),
    }
  }
}

impl From<String> for AssetSource {
  fn from(value: String) -> Self {
    Self::String(value)
  }
}

impl From<Vec<u8>> for AssetSource {
  fn from(value: Vec<u8>) -> Self {
    Self::Buffer(value)
  }
}
