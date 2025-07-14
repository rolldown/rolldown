use std::fmt::{Debug, Display};

#[derive(Clone, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct BitSet {
  entries: Vec<u8>,
  length: u32,
}

impl BitSet {
  pub fn new(max_bit_count: u32) -> Self {
    Self { entries: vec![0; max_bit_count.div_ceil(8) as usize], length: max_bit_count }
  }

  pub fn has_bit(&self, bit: u32) -> bool {
    (self.entries[bit as usize / 8] & (1 << (bit & 7))) != 0
  }

  pub fn set_bit(&mut self, bit: u32) {
    self.entries[bit as usize / 8] |= 1 << (bit & 7);
  }

  pub fn is_empty(&self) -> bool {
    self.entries.iter().all(|&e| e == 0)
  }

  #[expect(clippy::iter_without_into_iter)]
  pub fn iter(&self) -> BitSetIter {
    BitSetIter::new(self)
  }

  pub fn union(&mut self, other: &Self) {
    for (i, &e) in other.entries.iter().enumerate() {
      self.entries[i] |= e;
    }
  }
  // It is safe to convert `usize` to `u32` here because we ensure that the bitset is created with a maximum bit count that fits within `u32`.
  #[allow(clippy::cast_possible_truncation)]
  pub fn index_of_one(&self) -> Vec<u32> {
    let mut result = Vec::new();
    for (i, &e) in self.entries.iter().enumerate() {
      for j in 0..8 {
        if e & (1 << j) != 0 {
          result.push((i * 8 + j) as u32);
        }
      }
    }
    result
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

pub struct BitSetIter<'a> {
  bitset: &'a BitSet,
  curr: u32
}

impl<'a> BitSetIter<'a> {
  fn new(bitset: &'a BitSet) -> Self {
    Self { bitset, curr: 0 }
  }
}

impl Iterator for BitSetIter<'_> {
  type Item = bool;

  fn next(&mut self) -> Option<Self::Item> {
    if self.curr >= self.bitset.length {
      return None;
    }

    let val = self.bitset.has_bit(self.curr);
    self.curr += 1;
    Some(val)
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
  fn index_of_one() {
    let mut bits = BitSet::new(16);
    bits.set_bit(1);
    bits.set_bit(5);
    bits.set_bit(8);
    bits.set_bit(10);
    bits.set_bit(13);
    bits.set_bit(15);

    assert_eq!(bits.index_of_one(), vec![1, 5, 8, 10, 13, 15]);
  }
}
