use std::marker::PhantomData;

use oxc_index::Idx;

use crate::BitSet;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct IndexBitSet<I: Idx> {
  inner: BitSet,
  _marker: PhantomData<I>,
}

impl<I: Idx> IndexBitSet<I> {
  const _ASSERT_IDX_FITS_U32: () = assert!(I::MAX <= u32::MAX as usize);

  #[inline]
  fn bit(idx: I) -> u32 {
    // asserted by the const assertion above
    #[expect(clippy::cast_possible_truncation)]
    {
      idx.index() as u32
    }
  }

  pub fn new(capacity: usize) -> Self {
    let () = Self::_ASSERT_IDX_FITS_U32;
    debug_assert!(capacity <= I::MAX, "IndexBitSet capacity exceeds maximum index of type");
    #[expect(clippy::cast_possible_truncation)]
    let inner = { BitSet::new(capacity as u32) };
    Self { inner, _marker: PhantomData }
  }

  pub fn has_bit(&self, idx: I) -> bool {
    self.inner.has_bit(Self::bit(idx))
  }

  pub fn set_bit(&mut self, idx: I) {
    self.inner.set_bit(Self::bit(idx));
  }

  pub fn clear_bit(&mut self, idx: I) {
    self.inner.clear_bit(Self::bit(idx));
  }

  pub fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }

  pub fn bit_count(&self) -> u32 {
    self.inner.bit_count()
  }

  pub fn index_of_one(&self) -> impl Iterator<Item = I> + '_ {
    self.inner.index_of_one().map(|i| I::from_usize(i as usize))
  }

  pub fn union(&mut self, other: &Self) {
    self.inner.union(&other.inner);
  }
}

impl<I: Idx> Extend<I> for IndexBitSet<I> {
  fn extend<T: IntoIterator<Item = I>>(&mut self, iter: T) {
    self.inner.extend(iter.into_iter().map(Self::bit));
  }
}

impl<I: Idx> IntoIterator for IndexBitSet<I> {
  type Item = I;
  type IntoIter = std::vec::IntoIter<I>;

  fn into_iter(self) -> Self::IntoIter {
    self.inner.into_iter().map(|i| I::from_usize(i as usize)).collect::<Vec<_>>().into_iter()
  }
}

impl<I: Idx> FromIterator<I> for IndexBitSet<I> {
  fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
    let () = Self::_ASSERT_IDX_FITS_U32;
    let inner = iter.into_iter().map(Self::bit).collect::<BitSet>();
    Self { inner, _marker: PhantomData }
  }
}

impl<I: Idx> std::fmt::Debug for IndexBitSet<I> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_set().entries(self.index_of_one()).finish()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  oxc_index::define_index_type! {
    struct TestIdx = u32;
  }

  #[test]
  fn basic_operations() {
    let mut set = IndexBitSet::<TestIdx>::new(16);
    assert!(set.is_empty());
    assert_eq!(set.bit_count(), 0);

    set.set_bit(TestIdx::from_usize(0));
    assert!(set.has_bit(TestIdx::from_usize(0)));
    assert_eq!(set.bit_count(), 1);

    set.set_bit(TestIdx::from_usize(5));
    set.set_bit(TestIdx::from_usize(10));
    assert_eq!(set.bit_count(), 3);

    set.clear_bit(TestIdx::from_usize(5));
    assert!(!set.has_bit(TestIdx::from_usize(5)));
    assert_eq!(set.bit_count(), 2);
  }

  #[test]
  fn index_of_one_and_collect() {
    let mut set = IndexBitSet::<TestIdx>::new(16);
    set.set_bit(TestIdx::from_usize(1));
    set.set_bit(TestIdx::from_usize(3));
    set.set_bit(TestIdx::from_usize(7));

    let items: Vec<TestIdx> = set.index_of_one().collect();
    assert_eq!(items, vec![TestIdx::from_usize(1), TestIdx::from_usize(3), TestIdx::from_usize(7)]);

    let collected: IndexBitSet<TestIdx> =
      vec![TestIdx::from_usize(2), TestIdx::from_usize(4)].into_iter().collect();
    assert!(collected.has_bit(TestIdx::from_usize(2)));
    assert!(collected.has_bit(TestIdx::from_usize(4)));
    assert_eq!(collected.bit_count(), 2);
  }

  #[test]
  fn into_iterator() {
    let mut set = IndexBitSet::<TestIdx>::new(8);
    set.set_bit(TestIdx::from_usize(2));
    set.set_bit(TestIdx::from_usize(6));

    let items: Vec<TestIdx> = set.into_iter().collect();
    assert_eq!(items, vec![TestIdx::from_usize(2), TestIdx::from_usize(6)]);
  }

  #[test]
  fn extend_trait() {
    let mut set = IndexBitSet::<TestIdx>::new(16);
    set.set_bit(TestIdx::from_usize(0));
    set.extend(vec![TestIdx::from_usize(3), TestIdx::from_usize(7)]);
    assert_eq!(set.bit_count(), 3);
    assert!(set.has_bit(TestIdx::from_usize(3)));
  }

  #[test]
  fn union_sets() {
    let mut a = IndexBitSet::<TestIdx>::new(16);
    a.set_bit(TestIdx::from_usize(1));
    a.set_bit(TestIdx::from_usize(3));

    let mut b = IndexBitSet::<TestIdx>::new(16);
    b.set_bit(TestIdx::from_usize(3));
    b.set_bit(TestIdx::from_usize(5));

    a.union(&b);
    assert_eq!(a.bit_count(), 3);
    assert!(a.has_bit(TestIdx::from_usize(1)));
    assert!(a.has_bit(TestIdx::from_usize(3)));
    assert!(a.has_bit(TestIdx::from_usize(5)));
  }
}
