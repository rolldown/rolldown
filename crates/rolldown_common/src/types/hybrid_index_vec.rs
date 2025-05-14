use oxc_index::{Idx, IndexVec};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub enum HybridIndexVec<I: Idx, T> {
  IndexVec(IndexVec<I, T>),
  Map(FxHashMap<I, T>),
}

impl<I: Idx, T> Default for HybridIndexVec<I, T> {
  fn default() -> Self {
    HybridIndexVec::IndexVec(IndexVec::default())
  }
}

impl<I: Idx, T> HybridIndexVec<I, T> {
  pub fn new() -> Self {
    HybridIndexVec::IndexVec(IndexVec::new())
  }

  pub fn is_index_vec(&self) -> bool {
    matches!(self, HybridIndexVec::IndexVec(_))
  }

  pub fn push(&mut self, item: T) -> I {
    match self {
      HybridIndexVec::IndexVec(v) => v.push(item),
      HybridIndexVec::Map(m) => {
        let idx = I::from_usize(m.len());
        m.insert(idx, item);
        idx
      }
    }
  }
  /// # Panic
  /// This method is only available for `Sparse Map` variant.
  pub fn insert(&mut self, i: I, item: T) {
    match self {
      HybridIndexVec::IndexVec(_) => unreachable!(),
      HybridIndexVec::Map(m) => {
        m.insert(i, item);
      }
    }
  }

  pub fn reserve(&mut self, additional: usize) {
    match self {
      HybridIndexVec::IndexVec(v) => v.reserve(additional),
      HybridIndexVec::Map(_m) => {
        // For a sparse index vec, preserve memory space is not necessary
      }
    }
  }

  /// # Panic
  /// Caller should ensure the index is exists in container.
  pub fn get_mut(&mut self, i: I) -> &mut T {
    match self {
      HybridIndexVec::IndexVec(index_vec) => &mut index_vec[i],
      HybridIndexVec::Map(map) => map.get_mut(&i).expect("should have idx"),
    }
  }

  /// # Panic
  /// Caller should ensure the index is exists in container.
  pub fn get(&self, i: I) -> &T {
    match self {
      HybridIndexVec::IndexVec(index_vec) => &index_vec[i],
      HybridIndexVec::Map(map) => map.get(&i).expect("should have idx"),
    }
  }

  pub fn clear(&mut self) {
    match self {
      HybridIndexVec::IndexVec(vec) => vec.clear(),
      HybridIndexVec::Map(map) => map.clear(),
    }
  }

  pub fn into_iter_enumerated(self) -> impl IntoIterator<Item = (I, T)> {
    match self {
      HybridIndexVec::IndexVec(vec) => {
        itertools::Either::Left(vec.into_iter().enumerate().map(|(i, t)| (I::from_usize(i), t)))
      }
      HybridIndexVec::Map(map) => itertools::Either::Right(map.into_iter()),
    }
  }

  pub fn iter(&self) -> impl Iterator<Item = &T> {
    match self {
      HybridIndexVec::IndexVec(index_vec) => itertools::Either::Left(index_vec.iter()),
      HybridIndexVec::Map(hash_map) => itertools::Either::Right(hash_map.values()),
    }
  }
}
