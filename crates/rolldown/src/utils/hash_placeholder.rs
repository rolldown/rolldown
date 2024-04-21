use std::borrow::Cow;

use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use rustc_hash::FxHashMap;

const HASH_PLACEHOLDER_LEFT: &str = "!~{";
const HASH_PLACEHOLDER_RIGHT: &str = "}~";
const HASH_PLACEHOLDER_OVERHEAD: usize = HASH_PLACEHOLDER_LEFT.len() + HASH_PLACEHOLDER_RIGHT.len();

// This is the size of a 128-bits xxhash with base64url encoding
const MAX_HASH_SIZE: usize = 22;
// const DEFAULT_HASH_SIZE: usize = 8;

static REPLACER_REGEX: Lazy<Regex> = Lazy::new(|| {
  // let pattern = [HASH_PLACEHOLDER_LEFT, "[0-9a-zA-Z_$]{1,17}", HASH_PLACEHOLDER_RIGHT].concat();
  let pattern = "!~\\{[0-9a-zA-Z_$]{1,17}\\}~";
  Regex::new(pattern).expect("failed to compile regex")
});

const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";
const BASE: u32 = 64;

fn to_base64(mut value: u32) -> String {
  let mut out_string = String::new();
  loop {
    let current_digit = value % BASE;
    value /= BASE;
    out_string.push(CHARS[current_digit as usize] as char);
    if value == 0 {
      break;
    }
  }
  out_string
}

#[derive(Debug, Default)]
pub struct HashPlaceholderGenerator {
  seed: u32,
}

impl HashPlaceholderGenerator {
  pub fn generate(&mut self, len: usize) -> String {
    debug_assert!((HASH_PLACEHOLDER_OVERHEAD..=MAX_HASH_SIZE).contains(&len));
    let allow_middle_len = len - HASH_PLACEHOLDER_OVERHEAD;
    let mut seed_base64 = to_base64(self.seed);
    if seed_base64.len() > allow_middle_len {
      // TODO(hyf0): improve this
      panic!("seed is too large")
    } else {
      let mut padding = String::new();
      for _ in 0..(allow_middle_len - seed_base64.len()) {
        padding.push('0');
      }
      seed_base64 = [padding, seed_base64].concat();
    }
    self.seed += 1;
    let placeholder = format!("{HASH_PLACEHOLDER_LEFT}{seed_base64}{HASH_PLACEHOLDER_RIGHT}");

    placeholder
  }
}

pub fn replace_facade_hash_replacement(
  source: String,
  final_hashes_by_placeholder: &FxHashMap<String, &str>,
) -> String {
  let replaced = REPLACER_REGEX.replace_all(&source, |captures: &Captures<'_>| -> &str {
    debug_assert!(captures.len() == 1);
    let facade = captures.get(0).unwrap().as_str();
    let real_hash = final_hashes_by_placeholder.get(facade).unwrap_or_else(|| {
      panic!("This should not happen. hash not found for facade replacement: {facade}")
    });
    real_hash
  });

  match replaced {
    Cow::Borrowed(_) => source,
    Cow::Owned(s) => s,
  }
}

#[test]
fn test_facade_hash_generator() {
  let mut gen = HashPlaceholderGenerator::default();
  assert_eq!(gen.generate(8), "!~{000}~");
  assert_eq!(gen.generate(8), "!~{001}~");
}

#[test]
fn test_to_base64() {
  assert_eq!(to_base64(0), "0");
  assert_eq!(to_base64(1), "1");
  assert_eq!(to_base64(10), "a");
  assert_eq!(to_base64(64), "01");
  assert_eq!(to_base64(65), "11");
  assert_eq!(to_base64(128), "02");
  assert_eq!(to_base64(100_000_000), "04uZ5");
}
