use oxc_index::{Idx, IndexVec};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub enum HybridIndexVec<I: Idx, T: Default> {
  IndexVec(IndexVec<I, T>),
  Map(FxHashMap<I, T>),
}

impl<I: Idx, T: Default> Default for HybridIndexVec<I, T> {
  fn default() -> Self {
    HybridIndexVec::IndexVec(Default::default())
  }
}

impl<I: Idx, T: Default> HybridIndexVec<I, T> {
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

  pub fn get_mut(&mut self, i: I) -> &mut T {
    match self {
      HybridIndexVec::IndexVec(index_vec) => &mut index_vec[i],
      HybridIndexVec::Map(map) => {
        let ret = map.entry(i).or_insert(T::default());
        ret
      }
    }
  }

  pub fn clear(&mut self) {
    match self {
      HybridIndexVec::IndexVec(vec) => vec.clear(),
      HybridIndexVec::Map(map) => map.clear(),
    }
  }
}
