use oxc::span::{Atom, CompactStr};

use crate::Rstr;

pub trait ToRstr {
  fn to_rstr(&self) -> Rstr;
}

impl ToRstr for Atom<'_> {
  fn to_rstr(&self) -> Rstr {
    Rstr(CompactStr::new(self.as_str()))
  }
}

impl ToRstr for &str {
  fn to_rstr(&self) -> Rstr {
    Rstr(CompactStr::new(self))
  }
}

impl ToRstr for CompactStr {
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
