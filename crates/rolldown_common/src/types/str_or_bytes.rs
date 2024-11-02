use std::fmt::Debug;

#[derive(Clone, Debug)]
pub enum StrOrBytes {
  Str(String),
  Bytes(Vec<u8>),
}

impl Default for StrOrBytes {
  fn default() -> Self {
    Self::Str(String::default())
  }
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

  pub fn into_bytes(self) -> Vec<u8> {
    match self {
      Self::Str(s) => s.into_bytes(),
      Self::Bytes(b) => b,
    }
  }

  pub fn as_bytes(&self) -> &[u8] {
    match self {
      Self::Str(s) => s.as_bytes(),
      Self::Bytes(b) => b.as_slice(),
    }
  }

  pub fn try_to_str(&self) -> anyhow::Result<&str> {
    match self {
      Self::Str(s) => Ok(s.as_str()),
      Self::Bytes(b) => Ok(std::str::from_utf8(b.as_slice())?),
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
