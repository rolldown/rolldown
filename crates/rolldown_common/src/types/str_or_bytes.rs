#[derive(Clone)]
pub enum StrOrBytes {
  Str(String),
  Bytes(Vec<u8>),
}

impl StrOrBytes {
  pub fn try_into_string(self) -> anyhow::Result<String> {
    match self {
      Self::Str(s) => Ok(s),
      Self::Bytes(b) => Ok(String::from_utf8(b)?),
    }
  }

  pub fn try_into_bytes(self) -> anyhow::Result<Vec<u8>> {
    match self {
      Self::Str(s) => Ok(s.into_bytes()),
      Self::Bytes(b) => Ok(b),
    }
  }
}

impl From<String> for StrOrBytes {
  fn from(s: String) -> Self {
    Self::Str(s)
  }
}

impl From<Vec<u8>> for StrOrBytes {
  fn from(b: Vec<u8>) -> Self {
    Self::Bytes(b)
  }
}
