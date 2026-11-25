use rolldown_utils::base64::{from_url_safe_base64, to_url_safe_base64};

/// The longest LEB128 varint of a `u32`.
const MAX_VARINT_LEN: usize = 5;

/// A set of module numbers (see `HmrSession`). On the wire it is the shorter of two forms, each
/// base64url without padding after a one-character prefix (equal length picks `b`):
///
/// - `b`: a bitset. Bit `i` is set when number `i` is in the set, bytes in little-endian bit
///   order (bit `i` is `bytes[i / 8] >> (i % 8) & 1`).
/// - `d`: the sorted numbers as LEB128 varints of their gaps. The first varint is the first
///   number; each next one is the number minus the one before it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HmrNumSet {
  bytes: Vec<u8>,
}

impl HmrNumSet {
  pub fn from_nums(nums: impl IntoIterator<Item = u32>) -> Self {
    let mut set = Self::default();
    for num in nums {
      set.insert(num);
    }
    set
  }

  fn insert(&mut self, num: u32) {
    let byte = num as usize / 8;
    if self.bytes.len() <= byte {
      self.bytes.resize(byte + 1, 0);
    }
    self.bytes[byte] |= 1 << (num % 8);
  }

  /// `None` when `value` is not in the wire format, or when it names a number at or above
  /// `num_count`. The size of the result is bounded by `num_count`, whatever the input.
  pub fn decode(value: &str, num_count: u32) -> Option<Self> {
    let max_bytes = (num_count as usize).div_ceil(8);
    let (prefix, body) = value.split_at_checked(1)?;
    match prefix {
      "b" => {
        if body.len() > base64_len(max_bytes) {
          return None;
        }
        let bytes = from_url_safe_base64(body)?;
        let used_bits = num_count % 8;
        let fits = bytes.len() < max_bytes
          || (bytes.len() == max_bytes
            && (used_bits == 0 || bytes.last().is_none_or(|last| last >> used_bits == 0)));
        fits.then_some(Self { bytes })
      }
      "d" => {
        if body.len() > base64_len(num_count as usize * MAX_VARINT_LEN) {
          return None;
        }
        let gaps = from_url_safe_base64(body)?;
        let mut set = Self::default();
        let mut pos = 0;
        let mut previous = None;
        while pos < gaps.len() {
          let gap = read_varint(&gaps, &mut pos)?;
          let num = match previous {
            None => gap,
            // Gaps after the first are at least 1: the numbers strictly increase.
            Some(previous) if gap > 0 => u32::checked_add(previous, gap)?,
            Some(_) => return None,
          };
          if num >= num_count {
            return None;
          }
          set.insert(num);
          previous = Some(num);
        }
        Some(set)
      }
      _ => None,
    }
  }

  pub fn encode(&self) -> String {
    let len = self.bytes.iter().rposition(|byte| *byte != 0).map_or(0, |pos| pos + 1);
    let mut gaps = Vec::new();
    let mut previous = 0;
    for num in self.iter() {
      write_varint(&mut gaps, num - previous);
      previous = num;
    }
    // base64 grows with the byte count, so the fewer bytes give the shorter string.
    if gaps.len() < len {
      format!("d{}", to_url_safe_base64(&gaps))
    } else {
      format!("b{}", to_url_safe_base64(&self.bytes[..len]))
    }
  }

  pub fn is_empty(&self) -> bool {
    self.bytes.iter().all(|byte| *byte == 0)
  }

  pub fn contains(&self, num: u32) -> bool {
    self.bytes.get(num as usize / 8).is_some_and(|byte| (byte >> (num % 8)) & 1 == 1)
  }

  pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
    (0u32..).zip(&self.bytes).flat_map(|(index, byte)| {
      (0..8u32).filter(move |bit| (byte >> bit) & 1 == 1).map(move |bit| index * 8 + bit)
    })
  }
}

/// The length of `byte_len` bytes in base64 without padding.
fn base64_len(byte_len: usize) -> usize {
  (byte_len * 4).div_ceil(3)
}

/// `None` for a varint that is cut off, does not fit in a `u32`, or is longer than needed.
fn read_varint(bytes: &[u8], pos: &mut usize) -> Option<u32> {
  let mut value = 0u32;
  for index in 0..MAX_VARINT_LEN {
    let byte = *bytes.get(*pos)?;
    *pos += 1;
    let low = u32::from(byte & 0x7f);
    // The fifth byte holds only the top 4 bits of a `u32`.
    if index == MAX_VARINT_LEN - 1 && low > 0x0f {
      return None;
    }
    value |= low << (index * 7);
    if byte & 0x80 == 0 {
      // A last byte of 0 after the first adds nothing, so the varint is longer than needed.
      return (byte != 0 || index == 0).then_some(value);
    }
  }
  None
}

#[expect(clippy::cast_possible_truncation)]
fn write_varint(out: &mut Vec<u8>, mut value: u32) {
  while value >= 0x80 {
    out.push((value as u8 & 0x7f) | 0x80);
    value >>= 7;
  }
  out.push(value as u8);
}

/// What one client tab holds, as it describes itself in the query of a pull request:
/// `s=<epoch>&g=<generation>&u=<set>&l=<set>&e=<set>&f=<set>`. Other keys (the nonce `n`) are
/// ignored, and a missing set is empty.
#[derive(Debug, Default, Clone)]
pub struct ClientHave {
  /// `s`: the epoch of the engine whose output the client runs.
  pub epoch: u32,
  /// `g`: the generation up to which the client heard of every update. Every factory it holds
  /// is fresh as of it. A client that does not know the generation of its output leaves `g` out,
  /// and the dev server fills in the generation of the output it serves before the query gets
  /// here.
  pub generation: u32,
  /// `u`: the modules the client's update removes and runs again.
  pub update: HmrNumSet,
  /// `l`: the targets of the lazy `import()` calls the request is for. The server compiles a
  /// route that is not in the graph yet.
  pub lazy: HmrNumSet,
  /// `e`: the modules the client executed, minus `u`.
  pub executed: HmrNumSet,
  /// `f`: the modules whose factory the client holds, except those in `e`. The server never
  /// reads a factory of a module in `e`, because such a module keeps its exports.
  pub factories: HmrNumSet,
}

impl ClientHave {
  /// Accepts the query with or without the leading `?`, or a whole URL. `num_count` is the
  /// number of module numbers the session gave out; a set that names a higher number is
  /// rejected.
  pub fn parse_query(query: &str, num_count: u32) -> Result<Self, String> {
    let query = query.split_once('?').map_or(query, |(_, query)| query);
    let mut epoch = None;
    let mut generation = None;
    let mut have = Self::default();
    for pair in query.split('&') {
      let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
      let number = || value.parse::<u32>().map_err(|_| format!("`{key}` is not a number"));
      let set = || {
        HmrNumSet::decode(value, num_count)
          .ok_or_else(|| format!("`{key}` is not a set of known module numbers"))
      };
      match key {
        "s" => epoch = Some(number()?),
        "g" => generation = Some(number()?),
        "u" => have.update = set()?,
        "l" => have.lazy = set()?,
        "e" => have.executed = set()?,
        "f" => have.factories = set()?,
        _ => {}
      }
    }
    have.epoch = epoch.ok_or("the query has no `s`")?;
    have.generation = generation.ok_or("the query has no `g`")?;
    Ok(have)
  }

  /// The query the runtime builds for this have-set, without the nonce. Empty sets are left out.
  pub fn to_query(&self) -> String {
    let mut query = format!("s={}&g={}", self.epoch, self.generation);
    for (key, set) in
      [("u", &self.update), ("l", &self.lazy), ("e", &self.executed), ("f", &self.factories)]
    {
      if !set.is_empty() {
        query.push('&');
        query.push_str(key);
        query.push('=');
        query.push_str(&set.encode());
      }
    }
    query
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn encodes_the_shorter_form() {
    let set = HmrNumSet::from_nums([0, 3, 8, 17]);
    // Bitset: 09 01 02 (3 bytes). Gaps: 0, 3, 5, 9 (4 bytes). The bitset is shorter.
    assert_eq!(set.encode(), "bCQEC");
    // Gaps 5, 995, 9000: 05 e3 07 a8 46 (5 bytes). The bitset would be 1251 bytes.
    let sparse = HmrNumSet::from_nums([5, 1000, 10000]);
    assert_eq!(sparse.encode(), "dBeMHqEY");
    // Bitset 08, gap 03: one byte each, and equal length picks the bitset.
    assert_eq!(HmrNumSet::from_nums([3]).encode(), "bCA");
    for set in [set, sparse, HmrNumSet::from_nums(0..2000), HmrNumSet::default()] {
      let decoded = HmrNumSet::decode(&set.encode(), 10_001).unwrap();
      assert_eq!(decoded.iter().collect::<Vec<_>>(), set.iter().collect::<Vec<_>>());
    }
  }

  #[test]
  fn decodes_both_forms() {
    let bitset = HmrNumSet::decode("bCQEC", 18).unwrap();
    assert_eq!(bitset.iter().collect::<Vec<_>>(), [0, 3, 8, 17]);
    assert!(bitset.contains(17) && !bitset.contains(16) && !bitset.contains(1000));
    let gaps = HmrNumSet::decode("dAAMFCQ", 18).unwrap();
    assert_eq!(gaps, bitset);
    // A varint of two bytes: 128 = 80 01.
    assert_eq!(HmrNumSet::decode("dgAE", 129).unwrap().iter().collect::<Vec<_>>(), [128]);
  }

  #[test]
  fn rejects_hostile_sets() {
    let decode = |value: &str, num_count: u32| HmrNumSet::decode(value, num_count);
    // No prefix, an unknown prefix, bad base64.
    assert_eq!(decode("", 100), None);
    assert_eq!(decode("CQEC", 100), None);
    assert_eq!(decode("xCQEC", 100), None);
    assert_eq!(decode("b***", 100), None);
    assert_eq!(decode("\u{e9}", 100), None);
    // Number 17 is not below 17. A bitset longer than the session needs.
    assert_eq!(decode("bCQEC", 17), None);
    assert_eq!(decode("bCQECAA", 18), None);
    assert_eq!(decode("dAAMFCQ", 17), None);
    // A gap of 0 repeats a number.
    assert_eq!(decode(&format!("d{}", to_url_safe_base64([1, 0])), 100), None);
    // A varint that is cut off, one longer than needed, one too large for a `u32`.
    assert_eq!(decode(&format!("d{}", to_url_safe_base64([0x80])), 100), None);
    assert_eq!(decode(&format!("d{}", to_url_safe_base64([0x81, 0x00])), 100), None);
    let too_large = to_url_safe_base64([0xff, 0xff, 0xff, 0xff, 0x1f]);
    assert_eq!(decode(&format!("d{too_large}"), u32::MAX), None);
    // Only the fifth-byte check refuses 2^32: `<<` would drop the high bit and give 0.
    let two_pow_32 = to_url_safe_base64([0x80, 0x80, 0x80, 0x80, 0x10]);
    assert_eq!(decode(&format!("d{two_pow_32}"), 100), None);
    let six_bytes = to_url_safe_base64([0x80, 0x80, 0x80, 0x80, 0x80, 0x01]);
    assert_eq!(decode(&format!("d{six_bytes}"), u32::MAX), None);
    // Input much longer than any valid set of the session is refused before decoding.
    assert_eq!(decode(&format!("b{}", "A".repeat(10_000)), 100), None);
    assert_eq!(decode(&format!("d{}", "A".repeat(10_000)), 100), None);
    // Every number is below the count, so nothing needs to be named.
    assert_eq!(decode("b", 0), Some(HmrNumSet::default()));
    assert_eq!(decode("bAQ", 0), None);
    assert_eq!(decode("dAA", 0), None);
  }

  #[test]
  fn parses_the_patch_query() {
    let have =
      ClientHave::parse_query("/__rolldown_hmr_patch.js?s=42&g=3&u=bAg&l=bAg&f=dAA&n=x1", 2)
        .expect("valid query");
    assert_eq!((have.epoch, have.generation), (42, 3));
    assert_eq!(have.update.iter().collect::<Vec<_>>(), [1]);
    assert_eq!(have.lazy.iter().collect::<Vec<_>>(), [1]);
    assert!(have.executed.is_empty());
    assert_eq!(have.factories.iter().collect::<Vec<_>>(), [0]);
    assert_eq!(have.to_query(), "s=42&g=3&u=bAg&l=bAg&f=bAQ");
    assert_eq!(ClientHave::parse_query(&have.to_query(), 2).unwrap().factories, have.factories);
  }

  #[test]
  fn rejects_bad_queries() {
    ClientHave::parse_query("g=1", 10).unwrap_err();
    ClientHave::parse_query("s=1", 10).unwrap_err();
    ClientHave::parse_query("s=x&g=1", 10).unwrap_err();
    ClientHave::parse_query("s=1&g=1&u=b***", 10).unwrap_err();
    ClientHave::parse_query("s=1&g=1&u=dAAA", 10).unwrap_err();
    ClientHave::parse_query("s=1&g=1&e=", 10).unwrap_err();
    ClientHave::parse_query("s=1&g=1&l=bAQ", 0).unwrap_err();
    ClientHave::parse_query("s=1&g=1&f=bAQ", 0).unwrap_err();
  }
}
