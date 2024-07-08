use oxc::{
  allocator::{self, Allocator, Box, Vec},
  span::Atom,
};

use crate::TakeIn;

pub trait AllocatorExt<'alloc> {
  fn take<T: TakeIn<'alloc>>(&'alloc self, dest: &mut T) -> T;

  fn boxed<T>(&'alloc self, value: T) -> Box<'alloc, T>;

  fn new_vec_with<T>(&'alloc self, value: T) -> Vec<'alloc, T>;
  fn new_vec_with2<T>(&'alloc self, first: T, second: T) -> Vec<'alloc, T>;

  fn atom(&'alloc self, value: &str) -> Atom<'alloc>;
}

impl<'alloc> AllocatorExt<'alloc> for Allocator {
  fn take<T: TakeIn<'alloc>>(&'alloc self, dest: &mut T) -> T {
    std::mem::replace(dest, T::dummy(self))
  }

  fn boxed<T>(&'alloc self, value: T) -> Box<'alloc, T> {
    Box::new_in(value, self)
  }

  fn new_vec_with<T>(&'alloc self, value: T) -> Vec<'alloc, T> {
    let mut v = Vec::with_capacity_in(1, self);
    v.push(value);
    v
  }

  fn new_vec_with2<T>(&'alloc self, first: T, second: T) -> Vec<'alloc, T> {
    let mut v = Vec::with_capacity_in(2, self);
    v.push(first);
    v.push(second);
    v
  }
  fn atom(&'alloc self, value: &str) -> Atom<'alloc> {
    let alloc_str = allocator::String::from_str_in(value, self).into_bump_str();
    Atom::from(alloc_str)
  }
}
