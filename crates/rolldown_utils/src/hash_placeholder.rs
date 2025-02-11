use std::borrow::Cow;

use arcstr::ArcStr;
use regex::{Captures, Regex};
use rustc_hash::FxHashMap;
use std::sync::LazyLock;

use crate::indexmap::FxIndexSet;

const HASH_PLACEHOLDER_LEFT: &str = "!~{";
const HASH_PLACEHOLDER_RIGHT: &str = "}~";
const HASH_PLACEHOLDER_OVERHEAD: usize = HASH_PLACEHOLDER_LEFT.len() + HASH_PLACEHOLDER_RIGHT.len();

// This is the size of a 128-bit xxhash with `base_encode::to_string`
const MAX_HASH_SIZE: usize = 21;
// const DEFAULT_HASH_SIZE: usize = 8;

static REPLACER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = "!~\\{[0-9a-zA-Z_$]{1,17}\\}~";
  Regex::new(pattern).expect("failed to compile regex")
});

const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";
const BASE: u32 = 64;

fn to_base64(mut value: u32) -> String {
  let mut buffer = vec![];

  loop {
    let current_digit = value % BASE;
    buffer.push(CHARS[current_digit as usize]);
    value /= BASE;

    if value == 0 {
      break;
    }
  }
  buffer.reverse();
  // SAFETY: `buffer` is base64 characters, it is valid utf8 characters
  unsafe { String::from_utf8_unchecked(buffer) }
}

#[derive(Debug, Default)]
pub struct HashPlaceholderGenerator {
  seed: u32,
}

impl HashPlaceholderGenerator {
  pub fn generate_one(&mut self, len: usize) -> String {
    debug_assert!((HASH_PLACEHOLDER_OVERHEAD..=MAX_HASH_SIZE).contains(&len));

    let allow_middle_len = len - HASH_PLACEHOLDER_OVERHEAD;
    let seed_base64 = to_base64(self.seed);

    // TODO(hyf0): improve this
    assert!(seed_base64.len() <= allow_middle_len, "seed is too large");

    let mut placeholder =
      String::with_capacity(len + HASH_PLACEHOLDER_LEFT.len() + HASH_PLACEHOLDER_RIGHT.len());
    placeholder.push_str(HASH_PLACEHOLDER_LEFT);
    placeholder.extend(std::iter::repeat('0').take(allow_middle_len - seed_base64.len()));
    placeholder.push_str(&seed_base64);
    placeholder.push_str(HASH_PLACEHOLDER_RIGHT);

    self.seed += 1;

    placeholder
  }

  pub fn generate(&mut self, lens: Vec<usize>) -> Vec<String> {
    lens.into_iter().map(|len| self.generate_one(len)).collect()
  }
}

/// This function would replace the facade hash placeholder in the given input
/// ```js
/// import { foo } from "foo.!~{000}~.js";
/// ```
/// to
/// ```js
/// import { foo } from "foo.xx__hash.js";
/// ```
#[expect(clippy::implicit_hasher)]
pub fn replace_placeholder_with_hash<'a>(
  source: impl Into<Cow<'a, str>>,
  final_hashes_by_placeholder: &FxHashMap<ArcStr, &'a str>,
) -> Cow<'a, str> {
  let source = source.into();
  let replaced = REPLACER_REGEX.replace_all(&source, |captures: &Captures<'_>| -> ArcStr {
    debug_assert!(captures.len() == 1);
    // Eg. `!~{000}~`
    let captured_hash_placeholder = captures.get(0).unwrap().as_str();
    // If this is a unknown hash placeholder, we just keep it as is as rollup did in
    // https://github.com/rollup/rollup/blob/master/src/utils/hashPlaceholders.ts#L52

    let replacement = final_hashes_by_placeholder
      .get(captured_hash_placeholder)
      .unwrap_or(&captured_hash_placeholder);
    (*replacement).into()
  });

  if let Cow::Owned(owned) = replaced {
    // Due to the rustc's borrow checker, we can't return `replaced` directly
    owned.into()
  } else {
    // No replacement happened, return the original source
    source
  }
}

pub fn extract_hash_placeholders(source: &str) -> FxIndexSet<ArcStr> {
  let captures = REPLACER_REGEX.find_iter(source);
  captures.into_iter().map(|c| c.as_str().into()).collect()
}

#[test]
fn test_facade_hash_generator() {
  let mut gen = HashPlaceholderGenerator::default();
  assert_eq!(gen.generate(vec![8, 8]), vec!["!~{000}~", "!~{001}~"]);
  assert_eq!(gen.generate(vec![8, 8]), vec!["!~{002}~", "!~{003}~"]);
}

#[test]
fn test_to_base64() {
  assert_eq!(to_base64(0), "0");
  assert_eq!(to_base64(1), "1");
  assert_eq!(to_base64(10), "a");
  assert_eq!(to_base64(64), "10");
  assert_eq!(to_base64(65), "11");
  assert_eq!(to_base64(128), "20");
  assert_eq!(to_base64(100_000_000), "5Zu40");
}
