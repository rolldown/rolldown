//! `Rstr`:
//! - is meant to be a bundler-specialized string type for rolldown.
//! - to smooth integration with `oxc`'s string types.

use std::{borrow::Borrow, fmt::Display, ops::Deref};

/// `OxcStr` is a alias of string type oxc used internally.
pub type OxcStr = oxc::span::CompactStr;

mod to_str;
pub use to_str::ToRstr;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Rstr(OxcStr);

impl PartialOrd for Rstr {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.as_str().cmp(other.as_str()))
  }
}

impl Ord for Rstr {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.as_str().cmp(other.as_str())
  }
}

impl Rstr {
  pub fn new(s: &str) -> Self {
    Self(OxcStr::new(s))
  }

  pub fn inner(&self) -> &OxcStr {
    &self.0
  }

  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }
}

impl Default for Rstr {
  fn default() -> Self {
    Self(OxcStr::new(""))
  }
}

impl Deref for Rstr {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.as_str()
  }
}

impl AsRef<str> for Rstr {
  fn as_ref(&self) -> &str {
    self.0.as_str()
  }
}

impl Display for Rstr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.as_str().fmt(f)
  }
}

impl Borrow<str> for Rstr {
  fn borrow(&self) -> &str {
    self.as_str()
  }
}
