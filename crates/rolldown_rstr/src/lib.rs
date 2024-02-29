use std::{fmt::Display, ops::Deref};

use oxc::span::Atom;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Rstr(Atom);

impl Rstr {
  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }

  pub fn to_oxc_atom(&self) -> Atom {
    self.0.clone()
  }
}

impl Default for Rstr {
  fn default() -> Self {
    Self(Atom::new_inline(""))
  }
}

impl Deref for Rstr {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.as_str()
  }
}

impl Display for Rstr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.as_str().fmt(f)
  }
}

pub trait ToRstr {
  fn to_rstr(&self) -> Rstr;
}

impl ToRstr for Atom {
  fn to_rstr(&self) -> Rstr {
    Rstr(self.clone())
  }
}

impl From<&str> for Rstr {
  fn from(s: &str) -> Self {
    Self(s.into())
  }
}

impl From<String> for Rstr {
  fn from(s: String) -> Self {
    Self(s.into())
  }
}
