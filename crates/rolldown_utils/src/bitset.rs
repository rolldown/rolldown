use std::fmt::{Debug, Display};

#[derive(Clone, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct BitSet {
  entries: Vec<u8>,
}

impl BitSet {
  pub fn new(max_bit_count: u32) -> Self {
    Self { entries: vec![0; max_bit_count.div_ceil(8) as usize] }
  }

  pub fn all(max_bit_count: u32) -> Self {
    let mut entries = vec![u8::MAX; max_bit_count.div_ceil(8) as usize];
    if let Some(last) = entries.last_mut()
      && !max_bit_count.is_multiple_of(8)
    {
      *last = (1_u8 << (max_bit_count % 8)) - 1;
    }
    Self { entries }
  }

  pub fn has_bit(&self, bit: u32) -> bool {
    let idx = bit as usize / 8;
    if idx >= self.entries.len() {
      return false;
    }
    (self.entries[idx] & (1 << (bit & 7))) != 0
  }

  pub fn set_bit(&mut self, bit: u32) -> bool {
    let byte = &mut self.entries[bit as usize / 8];
    let mask = 1 << (bit & 7);
    let was_set = *byte & mask != 0;
    *byte |= mask;
    !was_set
  }

  pub fn clear_bit(&mut self, bit: u32) {
    self.entries[bit as usize / 8] &= !(1 << (bit & 7));
  }

  pub fn bit_count(&self) -> u32 {
    self.entries.iter().map(|e| e.count_ones()).sum()
  }

  pub fn is_empty(&self) -> bool {
    self.entries.iter().all(|&e| e == 0)
  }

  pub fn union(&mut self, other: &Self) {
    for (i, &e) in other.entries.iter().enumerate() {
      self.entries[i] |= e;
    }
  }

  pub fn intersect(&mut self, other: &Self) {
    for (left, right) in self.entries.iter_mut().zip(&other.entries) {
      *left &= right;
    }
    if self.entries.len() > other.entries.len() {
      self.entries[other.entries.len()..].fill(0);
    }
  }

  // It is safe to convert `usize` to `u32` here because we ensure that the bitset is created with a maximum bit count that fits within `u32`.
  #[expect(clippy::cast_possible_truncation)]
  pub fn index_of_one(&self) -> impl Iterator<Item = u32> + '_ {
    self.entries.iter().enumerate().flat_map(|(i, &e)| {
      (0..8u32).filter(move |&j| e & (1 << j) != 0).map(move |j| (i as u32) * 8 + j)
    })
  }
}

impl Extend<u32> for BitSet {
  fn extend<T: IntoIterator<Item = u32>>(&mut self, iter: T) {
    for bit in iter {
      self.set_bit(bit);
    }
  }
}

impl IntoIterator for BitSet {
  type Item = u32;
  type IntoIter = std::vec::IntoIter<u32>;

  fn into_iter(self) -> Self::IntoIter {
    self.index_of_one().collect::<Vec<_>>().into_iter()
  }
}

impl FromIterator<u32> for BitSet {
  fn from_iter<T: IntoIterator<Item = u32>>(iter: T) -> Self {
    let items: Vec<u32> = iter.into_iter().collect();
    if items.is_empty() {
      return Self::new(0);
    }
    let max = items.iter().copied().max().unwrap_or(0);
    let mut set = Self::new(max + 1);
    for bit in items {
      set.set_bit(bit);
    }
    set
  }
}

impl Display for BitSet {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // using little endian representation
    // e.g. 256
    // 00000001_00000000
    // ^               ^
    // msb             lsb
    let bit_string =
      self.entries.iter().rev().map(|e| format!("{e:08b}")).collect::<Vec<String>>().join("_");
    f.write_str(&bit_string)
  }
}

impl Debug for BitSet {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("BitSet").field(&self.to_string()).finish()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn basic() {
    let mut bs = BitSet::new(1);
    assert_eq!(bs.to_string(), "00000000");
    bs.set_bit(0);
    bs.set_bit(1);
    bs.set_bit(7);
    assert_eq!(bs.to_string(), "10000011");

    let mut bs = BitSet::new(9);
    assert_eq!(bs.to_string(), "00000000_00000000");
    bs.set_bit(0);
    bs.set_bit(1);
    bs.set_bit(7);
    assert_eq!(bs.to_string(), "00000000_10000011");
    bs.set_bit(8);
    assert_eq!(bs.to_string(), "00000001_10000011");
    bs.set_bit(15);
    assert_eq!(bs.to_string(), "10000001_10000011");
  }

  #[test]
  fn union() {
    let mut bs = BitSet::new(9);
    assert_eq!(bs.to_string(), "00000000_00000000");
    let mut bs2 = bs.clone();
    bs.set_bit(0);
    bs.set_bit(1);
    bs.set_bit(7);
    assert_eq!(bs.to_string(), "00000000_10000011");
    bs2.set_bit(8);
    bs2.set_bit(15);
    assert_eq!(bs2.to_string(), "10000001_00000000");
    //
    bs.union(&bs2);
    assert_eq!(bs.to_string(), "10000001_10000011");
  }

  #[test]
  fn all() {
    assert_eq!(BitSet::all(0).to_string(), "");
    assert_eq!(BitSet::all(1).to_string(), "00000001");
    assert_eq!(BitSet::all(8).to_string(), "11111111");
    assert_eq!(BitSet::all(9).to_string(), "00000001_11111111");
  }

  #[test]
  fn intersect() {
    let mut bs = BitSet::all(16);
    let mut bs2 = BitSet::new(16);
    bs2.set_bit(1);
    bs2.set_bit(8);
    bs2.set_bit(15);
    bs.intersect(&bs2);
    assert_eq!(bs.index_of_one().collect::<Vec<_>>(), vec![1, 8, 15]);
  }

  #[test]
  fn index_of_one() {
    let mut bits = BitSet::new(16);
    bits.set_bit(1);
    bits.set_bit(5);
    bits.set_bit(8);
    bits.set_bit(10);
    bits.set_bit(13);
    bits.set_bit(15);

    assert_eq!(bits.index_of_one().collect::<Vec<_>>(), vec![1, 5, 8, 10, 13, 15]);
  }

  #[test]
  fn clear_bit() {
    let mut bs = BitSet::new(16);
    bs.set_bit(0);
    bs.set_bit(5);
    bs.set_bit(15);
    assert_eq!(bs.to_string(), "10000000_00100001");

    bs.clear_bit(5);
    assert_eq!(bs.to_string(), "10000000_00000001");
    assert!(!bs.has_bit(5));
    assert!(bs.has_bit(0));
    assert!(bs.has_bit(15));

    bs.clear_bit(0);
    bs.clear_bit(15);
    assert!(bs.is_empty());
  }

  #[test]
  fn bit_count() {
    let mut bs = BitSet::new(16);
    assert_eq!(bs.bit_count(), 0);

    bs.set_bit(0);
    assert_eq!(bs.bit_count(), 1);

    bs.set_bit(7);
    bs.set_bit(8);
    bs.set_bit(15);
    assert_eq!(bs.bit_count(), 4);

    bs.clear_bit(7);
    assert_eq!(bs.bit_count(), 3);
  }

  #[test]
  fn extend() {
    let mut bs = BitSet::new(16);
    bs.extend([1, 5, 8]);
    assert_eq!(bs.bit_count(), 3);
    assert!(bs.has_bit(1));
    assert!(bs.has_bit(5));
    assert!(bs.has_bit(8));

    bs.extend([5, 15]);
    assert_eq!(bs.bit_count(), 4);
    assert!(bs.has_bit(15));
  }

  #[test]
  fn into_iter() {
    let mut bs = BitSet::new(16);
    bs.set_bit(2);
    bs.set_bit(7);
    bs.set_bit(13);

    let items: Vec<u32> = bs.into_iter().collect();
    assert_eq!(items, vec![2, 7, 13]);
  }

  #[test]
  fn from_iter() {
    let bs: BitSet = [3, 10, 15].into_iter().collect();
    assert_eq!(bs.bit_count(), 3);
    assert!(bs.has_bit(3));
    assert!(bs.has_bit(10));
    assert!(bs.has_bit(15));

    let empty: BitSet = std::iter::empty().collect();
    assert!(empty.is_empty());
  }

  #[test]
  fn has_bit_out_of_bounds() {
    let bs = BitSet::new(0);
    assert!(!bs.has_bit(0));

    let bs = BitSet::new(1);
    assert!(!bs.has_bit(100));
  }
}
