use oxc::span::{Atom, CompactString};

use crate::Rstr;

pub trait ToRstr {
  fn to_rstr(&self) -> Rstr;
}

impl ToRstr for Atom<'_> {
  fn to_rstr(&self) -> Rstr {
    match self {
      Atom::Arena(s) => Rstr((*s).to_string().into()),
      Atom::Compact(s) => Rstr(s.clone()),
    }
  }
}

impl ToRstr for CompactString {
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
