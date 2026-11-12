//! Segment file format. All numbers are little-endian.
//!
//! ```text
//! header   magic [8] | version u32 | 0u32
//! values   bytes of each value, one after another
//! footer   created_ms u64
//!          group count u32,  each: key1 u128 | last_used u64 | list count u32,
//!                                  each list: path count u32, each path: len u32 | utf-8 bytes
//!          result count u32, each: key1 u128 | key2 u128 | offset u64 | len u64 | checksum u64 | last_used u64
//!          list mark count u32,   each: key1 u128 | time u64
//!          result mark count u32, each: key1 u128 | key2 u128 | time u64
//! trailer  footer len u64 | footer checksum u64 | version u32 | 0u32 | magic [8]
//! ```
//!
//! Lists of one group are stored oldest first. A mark says that the entry was used at its time.
//! The entry can be in another segment.

use std::sync::Arc;

use xxhash_rust::xxh3::xxh3_64;

use crate::Key;

pub const MAGIC: [u8; 8] = *b"RDSCACHE";
pub const FORMAT_VERSION: u32 = 1;
pub const HEADER_LEN: usize = 16;
pub const TRAILER_LEN: usize = 32;

const KEY_LEN: usize = 16;
const RESULT_RECORD_LEN: usize = 2 * KEY_LEN + 4 * 8;
const LIST_MARK_LEN: usize = KEY_LEN + 8;
const RESULT_MARK_LEN: usize = 2 * KEY_LEN + 8;

pub struct ListGroup {
  pub key1: Key,
  pub last_used: u64,
  pub lists: Vec<Arc<[String]>>,
}

pub struct ResultRecord {
  pub key1: Key,
  pub key2: Key,
  pub offset: u64,
  pub len: u64,
  pub checksum: u64,
  pub last_used: u64,
}

/// `(key1, time)`.
pub type ListMark = (Key, u64);
/// `((key1, key2), time)`.
pub type ResultMark = ((Key, Key), u64);

pub struct Footer {
  pub created_ms: u64,
  pub groups: Vec<ListGroup>,
  pub results: Vec<ResultRecord>,
  pub list_marks: Vec<ListMark>,
  pub result_marks: Vec<ResultMark>,
}

pub fn header() -> [u8; HEADER_LEN] {
  let mut out = [0; HEADER_LEN];
  out[..8].copy_from_slice(&MAGIC);
  out[8..12].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
  out
}

pub fn trailer(footer: &[u8]) -> [u8; TRAILER_LEN] {
  let mut out = [0; TRAILER_LEN];
  out[..8].copy_from_slice(&(footer.len() as u64).to_le_bytes());
  out[8..16].copy_from_slice(&xxh3_64(footer).to_le_bytes());
  out[16..20].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
  out[24..].copy_from_slice(&MAGIC);
  out
}

/// Returns the footer length and checksum, or `None` for another format.
pub fn parse_trailer(trailer: &[u8]) -> Option<(u64, u64)> {
  let mut r = Reader(trailer);
  let footer_len = r.u64()?;
  let checksum = r.u64()?;
  let version = r.u32()?;
  r.u32()?;
  (version == FORMAT_VERSION && r.0 == MAGIC).then_some((footer_len, checksum))
}

pub fn encode_footer(footer: &Footer) -> Vec<u8> {
  let mut w = Vec::with_capacity(
    32 + footer.results.len() * RESULT_RECORD_LEN
      + footer.list_marks.len() * LIST_MARK_LEN
      + footer.result_marks.len() * RESULT_MARK_LEN,
  );
  put_u64(&mut w, footer.created_ms);
  put_len(&mut w, footer.groups.len());
  for group in &footer.groups {
    put_u128(&mut w, group.key1);
    put_u64(&mut w, group.last_used);
    put_len(&mut w, group.lists.len());
    for list in &group.lists {
      put_len(&mut w, list.len());
      for path in list.iter() {
        put_len(&mut w, path.len());
        w.extend_from_slice(path.as_bytes());
      }
    }
  }
  put_len(&mut w, footer.results.len());
  for rec in &footer.results {
    put_u128(&mut w, rec.key1);
    put_u128(&mut w, rec.key2);
    put_u64(&mut w, rec.offset);
    put_u64(&mut w, rec.len);
    put_u64(&mut w, rec.checksum);
    put_u64(&mut w, rec.last_used);
  }
  put_len(&mut w, footer.list_marks.len());
  for &(key1, time) in &footer.list_marks {
    put_u128(&mut w, key1);
    put_u64(&mut w, time);
  }
  put_len(&mut w, footer.result_marks.len());
  for &((key1, key2), time) in &footer.result_marks {
    put_u128(&mut w, key1);
    put_u128(&mut w, key2);
    put_u64(&mut w, time);
  }
  w
}

/// `values_end` is the file offset where the footer starts. Every value must end before it.
pub fn decode_footer(bytes: &[u8], checksum: u64, values_end: u64) -> Option<Footer> {
  if xxh3_64(bytes) != checksum {
    return None;
  }
  let mut r = Reader(bytes);
  let created_ms = r.u64()?;

  let count = r.count(KEY_LEN + 8 + 4)?;
  let mut groups = Vec::with_capacity(count);
  for _ in 0..count {
    let key1 = r.u128()?;
    let last_used = r.u64()?;
    let list_count = r.count(4)?;
    let mut lists = Vec::with_capacity(list_count);
    for _ in 0..list_count {
      let path_count = r.count(4)?;
      let mut paths = Vec::with_capacity(path_count);
      for _ in 0..path_count {
        let len = r.count(1)?;
        paths.push(String::from_utf8(r.bytes(len)?.to_vec()).ok()?);
      }
      lists.push(Arc::from(paths));
    }
    groups.push(ListGroup { key1, last_used, lists });
  }

  let count = r.count(RESULT_RECORD_LEN)?;
  let mut results = Vec::with_capacity(count);
  for _ in 0..count {
    let rec = ResultRecord {
      key1: r.u128()?,
      key2: r.u128()?,
      offset: r.u64()?,
      len: r.u64()?,
      checksum: r.u64()?,
      last_used: r.u64()?,
    };
    let end = rec.offset.checked_add(rec.len)?;
    if rec.offset < HEADER_LEN as u64 || end > values_end {
      return None;
    }
    results.push(rec);
  }

  let count = r.count(LIST_MARK_LEN)?;
  let mut list_marks = Vec::with_capacity(count);
  for _ in 0..count {
    list_marks.push((r.u128()?, r.u64()?));
  }

  let count = r.count(RESULT_MARK_LEN)?;
  let mut result_marks = Vec::with_capacity(count);
  for _ in 0..count {
    result_marks.push(((r.u128()?, r.u128()?), r.u64()?));
  }

  r.0.is_empty().then_some(Footer { created_ms, groups, results, list_marks, result_marks })
}

fn put_u64(w: &mut Vec<u8>, v: u64) {
  w.extend_from_slice(&v.to_le_bytes());
}

fn put_u128(w: &mut Vec<u8>, v: u128) {
  w.extend_from_slice(&v.to_le_bytes());
}

fn put_len(w: &mut Vec<u8>, len: usize) {
  // Lengths come from lists, paths and entry counts of one build. They fit in u32.
  w.extend_from_slice(&u32::try_from(len).unwrap_or(u32::MAX).to_le_bytes());
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
  fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
    if n > self.0.len() {
      return None;
    }
    let (head, rest) = self.0.split_at(n);
    self.0 = rest;
    Some(head)
  }

  fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
    self.bytes(N)?.try_into().ok()
  }

  fn u32(&mut self) -> Option<u32> {
    self.array().map(u32::from_le_bytes)
  }

  fn u64(&mut self) -> Option<u64> {
    self.array().map(u64::from_le_bytes)
  }

  fn u128(&mut self) -> Option<u128> {
    self.array().map(u128::from_le_bytes)
  }

  /// Reads a count and rejects it when the rest of the buffer cannot hold that many items of
  /// `min_item_len` bytes. This keeps damaged counts from causing huge allocations.
  fn count(&mut self, min_item_len: usize) -> Option<usize> {
    let count = usize::try_from(self.u32()?).ok()?;
    (count.checked_mul(min_item_len)? <= self.0.len()).then_some(count)
  }
}
