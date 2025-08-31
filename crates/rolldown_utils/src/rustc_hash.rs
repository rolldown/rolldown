use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

pub trait FxHashSetExt {
  fn with_capacity(capacity: usize) -> Self;
}

pub trait FxHashMapExt {
  fn with_capacity(capacity: usize) -> Self;
}
#[expect(clippy::implicit_hasher)]
impl<K, V> FxHashMapExt for FxHashMap<K, V> {
  fn with_capacity(capacity: usize) -> Self {
    FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher)
  }
}

#[expect(clippy::implicit_hasher)]
impl<T> FxHashSetExt for FxHashSet<T> {
  fn with_capacity(capacity: usize) -> Self {
    FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher)
  }
}
