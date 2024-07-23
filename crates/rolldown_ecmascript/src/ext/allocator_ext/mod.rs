use oxc::allocator::Allocator;

use crate::TakeIn;

pub trait AllocatorExt<'alloc> {
  fn take<T: TakeIn<'alloc>>(&'alloc self, dest: &mut T) -> T;
}

impl<'alloc> AllocatorExt<'alloc> for Allocator {
  fn take<T: TakeIn<'alloc>>(&'alloc self, dest: &mut T) -> T {
    std::mem::replace(dest, T::dummy(self))
  }
}
