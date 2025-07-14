use core::iter;
use std::{fmt, marker::PhantomData};

use oxc_index::{Idx, IndexVec};

use crate::{BitSet, bitset::BitSetIter};

type Enumerated<Iter, I, T> = iter::Map<iter::Enumerate<Iter>, fn((usize, T)) -> (I, T)>;

#[derive(Clone, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct IndexBitSet<I: Idx + Into<u32>> {
  raw: BitSet,
  _marker: PhantomData<fn(&I)>,
}

impl<I: Idx + Into<u32>> fmt::Display for IndexBitSet<I> {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(&self.raw, fmt)
  }
}

impl<I: Idx + Into<u32>> fmt::Debug for IndexBitSet<I> {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Debug::fmt(&self.raw, fmt)
  }
}

impl<I: Idx + Into<u32>> IndexBitSet<I> {
  #[inline]
  pub fn new(max_bit_count: u32) -> Self {
    Self { raw: BitSet::new(max_bit_count), _marker: PhantomData }
  }

  #[inline]
  pub fn has_bit(&self, bit: I) -> bool {
    self.raw.has_bit(bit.into())
  }

  #[inline]
  pub fn set_bit(&mut self, bit: I) {
    self.raw.set_bit(bit.into());
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.raw.is_empty()
  }

  #[expect(clippy::iter_without_into_iter)]
  #[inline]
  pub fn iter(&self) -> BitSetIter {
    self.raw.iter()
  }

  #[inline]
  pub fn iter_enumerated(&self) -> Enumerated<BitSetIter, I, bool> {
    self.raw.iter().enumerate().map(|(i, t)| (I::from_usize(i), t))
  }

  #[inline]
  pub fn union(&mut self, other: &Self) {
    self.raw.union(&other.raw);
  }

  #[inline]
  pub fn index_of_one(&self) -> IndexVec<I, u32> {
    self.raw.index_of_one().into()
  }
}
