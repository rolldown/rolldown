use oxc::allocator::Allocator;

use crate::DummyIn;

pub trait TakeIn<'ast> {
  /// A sugar function for using `std::mem::take` on `oxc_ast`.
  ///
  /// Replaces `self` with a [`Dummy`] value of `Self`, returning the previous value stored in `self`.
  fn take_in(&mut self, alloc: &'ast Allocator) -> Self;
}

impl<'ast, T: DummyIn<'ast>> TakeIn<'ast> for T {
  #[inline]
  fn take_in(&mut self, alloc: &'ast Allocator) -> Self {
    std::mem::replace(self, DummyIn::dummy_in(alloc))
  }
}
