use oxc::{
  allocator::{Allocator, Box, Vec},
  span::Span,
};

const DUMMY_SPAN: Span = Span::new(0, 0);

/// Create a Dummy value of `Self` in the passing `Allocator`.
pub trait DummyIn<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self;
}

impl<'ast> DummyIn<'ast> for Span {
  fn dummy_in(_alloc: &'ast Allocator) -> Self {
    DUMMY_SPAN
  }
}

impl<'ast, T: DummyIn<'ast>> DummyIn<'ast> for Box<'ast, T> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Box(alloc.alloc(DummyIn::dummy_in(alloc)))
  }
}

impl<'ast, T> DummyIn<'ast> for Vec<'ast, T> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Vec::new_in(alloc)
  }
}

impl<'ast, T> DummyIn<'ast> for Option<T> {
  fn dummy_in(_alloc: &'ast Allocator) -> Self {
    None
  }
}

impl<'ast> DummyIn<'ast> for bool {
  fn dummy_in(_alloc: &'ast Allocator) -> Self {
    false
  }
}
