use std::borrow::Cow;

use arcstr::ArcStr;
use rustc_hash::FxHashMap;

use crate::indexmap::FxIndexSet;

const HASH_PLACEHOLDER_LEFT: &str = "!~{";
const HASH_PLACEHOLDER_RIGHT: &str = "}~";
const HASH_PLACEHOLDER_OVERHEAD: usize = HASH_PLACEHOLDER_LEFT.len() + HASH_PLACEHOLDER_RIGHT.len();

// This is the size of a 128-bit xxhash with `base_encode::to_string`
const MIN_HASH_SIZE: usize = 6;
const MAX_HASH_SIZE: usize = 21;
const DEFAULT_HASH_SIZE: usize = 8;

/// Checks if a string is a hash placeholder with the pattern "!~{...}~"
/// where ... is 1-17 alphanumeric characters or _ or $
fn is_hash_placeholder(s: &str) -> bool {
  // Check if the string starts with the left placeholder and ends with the right placeholder
  if !s.starts_with(HASH_PLACEHOLDER_LEFT) || !s.ends_with(HASH_PLACEHOLDER_RIGHT) {
    return false;
  }

  // Extract the content between the placeholders
  let content = &s[HASH_PLACEHOLDER_LEFT.len()..s.len() - HASH_PLACEHOLDER_RIGHT.len()];

  // Content must be 1-17 characters long
  if content.is_empty() || content.len() > 17 {
    return false;
  }

  // All characters must be alphanumeric or _ or $
  content.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Finds all hash placeholders in a string and returns their positions and values
fn find_hash_placeholders(s: &str) -> Vec<(usize, usize, &str)> {
  // pre-allocate, the max number of placeholders is s.len() / 2
  let mut results = Vec::with_capacity(s.len() / 2);
  let mut start = 0;

  while let Some(left_pos) = s[start..].find(HASH_PLACEHOLDER_LEFT) {
    let left_pos = start + left_pos;
    if let Some(right_pos) = s[left_pos..].find(HASH_PLACEHOLDER_RIGHT) {
      let right_pos = left_pos + right_pos + HASH_PLACEHOLDER_RIGHT.len();
      let placeholder = &s[left_pos..right_pos];

      if is_hash_placeholder(placeholder) {
        results.push((left_pos, right_pos, placeholder));
      }

      start = right_pos;
    } else {
      break;
    }
  }

  results
}

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
  pub fn generate(&mut self, len: Option<usize>) -> String {
    // Ensure the generated hash length is within the valid range (6-21).
    // If `len` is `None`, default to 8.
    let len = len.map_or(DEFAULT_HASH_SIZE, |len| len.clamp(MIN_HASH_SIZE, MAX_HASH_SIZE));

    let allow_middle_len = len - HASH_PLACEHOLDER_OVERHEAD;
    let seed_base64 = to_base64(self.seed);

    // TODO(hyf0): improve this
    assert!(seed_base64.len() <= allow_middle_len, "seed is too large");

    let mut placeholder =
      String::with_capacity(len + HASH_PLACEHOLDER_LEFT.len() + HASH_PLACEHOLDER_RIGHT.len());
    placeholder.push_str(HASH_PLACEHOLDER_LEFT);
    placeholder.extend(std::iter::repeat_n('0', allow_middle_len - seed_base64.len()));
    placeholder.push_str(&seed_base64);
    placeholder.push_str(HASH_PLACEHOLDER_RIGHT);

    self.seed += 1;

    placeholder
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
  let source: Cow<'a, str> = source.into();

  // Check for placeholders directly
  let placeholders = find_hash_placeholders(&source);
  if placeholders.is_empty() {
    return source;
  }

  // Create a new string with replacements
  let mut result = String::with_capacity(source.len());
  let mut last_end = 0;

  for (start, end, placeholder) in placeholders {
    // Add the text before this placeholder
    result.push_str(&source[last_end..start]);

    // Add the replacement or the original placeholder if not found
    let replacement = final_hashes_by_placeholder.get(placeholder).unwrap_or(&placeholder);
    result.push_str(replacement);

    last_end = end;
  }

  // Add any remaining text
  if last_end < source.len() {
    result.push_str(&source[last_end..]);
  }

  Cow::Owned(result)
}

pub fn extract_hash_placeholders(source: &str) -> FxIndexSet<ArcStr> {
  let mut result = FxIndexSet::default();

  for (_, _, placeholder) in find_hash_placeholders(source) {
    result.insert(placeholder.into());
  }

  result
}

#[test]
fn test_facade_hash_generator() {
  let mut r#gen = HashPlaceholderGenerator::default();
  assert_eq!(r#gen.generate(None), "!~{000}~");
  assert_eq!(r#gen.generate(None), "!~{001}~");
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

#[test]
fn test_is_hash_placeholder() {
  assert!(is_hash_placeholder("!~{000}~"));
  assert!(is_hash_placeholder("!~{abc123}~"));
  assert!(is_hash_placeholder("!~{_$ABC123}~"));
  assert!(is_hash_placeholder("!~{12345678901234567}~")); // 17 chars

  assert!(!is_hash_placeholder("!~{}~")); // Empty content
  assert!(!is_hash_placeholder("!~{123456789012345678}~")); // 18 chars (too long)
  assert!(!is_hash_placeholder("!~{abc-123}~")); // Invalid char
  assert!(!is_hash_placeholder("{000}~")); // Missing left
  assert!(!is_hash_placeholder("!~{000}")); // Missing right
  assert!(!is_hash_placeholder("!~000}~")); // Missing {
}

#[test]
fn test_find_hash_placeholders() {
  let s = "prefix!~{000}~middle!~{abc}~suffix";
  let placeholders = find_hash_placeholders(s);
  assert_eq!(placeholders.len(), 2);
  assert_eq!(placeholders[0], (6, 14, "!~{000}~"));
  assert_eq!(placeholders[1], (20, 28, "!~{abc}~"));

  let s = "no placeholders here";
  let placeholders = find_hash_placeholders(s);
  assert_eq!(placeholders.len(), 0);

  let s = "!~{000}~!~{001}~";
  let placeholders = find_hash_placeholders(s);
  assert_eq!(placeholders.len(), 2);
  assert_eq!(placeholders[0], (0, 8, "!~{000}~"));
  assert_eq!(placeholders[1], (8, 16, "!~{001}~"));
}
