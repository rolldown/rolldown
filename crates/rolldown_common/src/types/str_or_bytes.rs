use std::fmt::Debug;

use arcstr::ArcStr;

#[derive(Clone, Debug)]
pub enum StrOrBytes {
  ArcStr(ArcStr),
  Str(String),
  // The second field is used to indicate whether the bytes have been validated as utf8
  Bytes(Vec<u8>, bool),
}

impl Default for StrOrBytes {
  fn default() -> Self {
    Self::Str(String::default())
  }
}
// Methods contain `inner` word won't do implicit conversion.
impl StrOrBytes {
  pub fn try_into_inner_string(self) -> anyhow::Result<String> {
    match self {
      Self::Str(s) => Ok(s),
      Self::ArcStr(s) => Ok(s.to_string()),
      Self::Bytes(..) => Err(anyhow::format_err!("Expected Str, found Bytes")),
    }
  }

  pub fn try_into_string(self) -> anyhow::Result<String> {
    match self {
      Self::Str(s) => Ok(s),
      Self::ArcStr(s) => Ok(s.to_string()),
      Self::Bytes(b, valid) => {
        if !valid {
          // validate utf8
          simdutf8::basic::from_utf8(&b)?;
        }
        // SAFETY: `b` is valid utf8
        unsafe { Ok(String::from_utf8_unchecked(b)) }
      }
    }
  }

  pub fn as_bytes(&self) -> &[u8] {
    match self {
      Self::Str(s) => s.as_bytes(),
      Self::ArcStr(s) => s.as_bytes(),
      Self::Bytes(b, _) => b.as_slice(),
    }
  }

  pub fn try_as_inner_str(&self) -> anyhow::Result<&str> {
    match self {
      Self::Str(s) => Ok(s.as_str()),
      Self::ArcStr(s) => Ok(s.as_str()),
      Self::Bytes(..) => Err(anyhow::format_err!("Expected Str, found Bytes")),
    }
  }

  pub fn to_str(&self) -> Option<&str> {
    match self {
      Self::Str(s) => Some(s.as_str()),
      Self::ArcStr(s) => Some(s.as_str()),
      Self::Bytes(..) => None,
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
    Self::Bytes(b, false)
  }
}
