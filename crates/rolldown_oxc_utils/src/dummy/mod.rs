use oxc::{
  allocator::{Allocator, Box, Vec},
  span::Span,
};

mod impl_for_ast_node;

const DUMMY_SPAN: Span = Span::new(0, 0);

/// Create a dummy value of `Self` in the passing `Allocator`.
pub trait Dummy<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self;
}

impl<'ast> Dummy<'ast> for Span {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    DUMMY_SPAN
  }
}

impl<'ast, T: Dummy<'ast>> Dummy<'ast> for Box<'ast, T> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Box(alloc.alloc(Dummy::dummy(alloc)))
  }
}

impl<'ast, T> Dummy<'ast> for Vec<'ast, T> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Vec::new_in(alloc)
  }
}

impl<'ast, T> Dummy<'ast> for Option<T> {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    None
  }
}

impl<'ast> Dummy<'ast> for bool {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    false
  }
}
