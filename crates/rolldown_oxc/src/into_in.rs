use oxc::allocator::{Allocator, Box};

use crate::from_in::FromIn;

pub trait IntoIn<'ast, T>: Sized {
  fn into_in(self, alloc: &'ast Allocator) -> T;
}

impl<'ast, T> IntoIn<'ast, Box<'ast, T>> for T {
  fn into_in(self, alloc: &'ast Allocator) -> Box<'ast, T> {
    Box(alloc.alloc(self))
  }
}

impl<'ast, T: FromIn<'ast, T>> IntoIn<'ast, T> for T {
  fn into_in(self, alloc: &'ast Allocator) -> T {
    T::from_in(self, alloc)
  }
}
