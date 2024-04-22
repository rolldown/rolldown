use oxc::allocator::{Allocator, Box};

pub trait FromIn<'ast, T>: Sized {
  fn from_in(value: T, alloc: &'ast Allocator) -> Self;
}

impl<'ast, T> FromIn<'ast, T> for Box<'ast, T> {
  fn from_in(value: T, alloc: &'ast Allocator) -> Self {
    Box::new_in(value, alloc)
  }
}
