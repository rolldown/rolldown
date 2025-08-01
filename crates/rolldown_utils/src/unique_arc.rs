use std::sync::{Arc, Weak};

use crate::debug::pretty_type_name;

#[derive(Debug)]
// Wrapper around Arc that ensures unique ownership while allowing weak references
pub struct UniqueArc<T: ?Sized>(Arc<T>);

impl<T> UniqueArc<T> {
  pub fn new(value: T) -> Self {
    Self(Arc::new(value))
  }

  /// # Panics
  /// - Don't call this method while calling the `WeakRef#with_inner` method.
  pub fn into_inner(self) -> T {
    Arc::into_inner(self.0).unwrap_or_else(|| {
      let t_name = pretty_type_name::<T>();
      panic!("UniqueArc<{t_name}> has multiple references")
    })
  }

  pub fn weak_ref(&self) -> WeakRef<T> {
    WeakRef(Arc::downgrade(&self.0))
  }
}

#[derive(Debug)]
// Weak reference that prevents storage of Arc to maintain unique ownership guarantees
pub struct WeakRef<T: ?Sized>(Weak<T>);

impl<T> WeakRef<T> {
  /// This API pattern is intended to prevent users from getting `Arc<T>` and storing it.
  /// This will cause `UniqueArc#into_inner` panic if `Arc<T>` got stored.
  pub fn try_with_inner<F, R>(&self, f: F) -> Option<R>
  where
    F: FnOnce(&T) -> R,
  {
    self.0.upgrade().map(|arc| f(&*arc))
  }

  pub fn with_inner<F, R>(&self, f: F) -> R
  where
    F: FnOnce(&T) -> R,
  {
    self.try_with_inner(f).unwrap_or_else(|| {
      let t_name = pretty_type_name::<T>();
      panic!(
        "UniqueArc<{t_name}> is already dropped. You can't access it by this WeakRef<{t_name}>",
      )
    })
  }
}

impl<T> Clone for WeakRef<T> {
  fn clone(&self) -> Self {
    Self(Weak::clone(&self.0))
  }
}
