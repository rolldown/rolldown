//! `Rstr`:
//! - is meant to be a bundler-specialized string type for rolldown.
//! - to smooth integration with `oxc`'s string types.

use std::{fmt::Display, ops::Deref};

use oxc::span::{Atom, CompactStr};

mod to_str;
pub use to_str::ToRstr;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Rstr(CompactStr);

impl Rstr {
  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }

  pub fn to_oxc_atom(&self) -> Atom<'static> {
    let va: Atom<'_> = self.as_str().into();
    unsafe { std::mem::transmute(va) }
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
