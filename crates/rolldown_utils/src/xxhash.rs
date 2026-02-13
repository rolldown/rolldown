// Copied from https://github.com/rollup/rollup/blob/080d2245ab6b6298229ebe7258c2b96816e7c52d/rust/xxhash/src/lib.rs

use base_encode::to_string;
use xxhash_rust::xxh3::xxh3_128;

use crate::base64::to_url_safe_base64;

pub fn xxhash_base64_url(input: &[u8]) -> String {
  let hash = xxh3_128(input).to_le_bytes();
  to_url_safe_base64(hash)
}

const CHARACTERS_BASE64: &[u8] =
  b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

const CHARACTERS_BASE16: &[u8] = b"0123456789abcdef";

/// Hash input with xxh3_128, then encode with the given base.
#[inline]
pub fn xxhash_with_base(input: &[u8], base: u8) -> String {
  encode_hash_with_base(&xxh3_128(input).to_le_bytes(), base)
}

/// Encode pre-hashed 128-bit digest with the given base.
///
/// Generates a base-encoded xxhash string for use in output filenames.
/// The result is guaranteed to be at least 21 characters (`MAX_HASH_SIZE`),
/// left-padded with the zero character of the given base when necessary.
pub fn encode_hash_with_base(hash: &[u8; 16], base: u8) -> String {
  let chars = match base {
    64 => CHARACTERS_BASE64,
    36 => &CHARACTERS_BASE64[26..(26 + 36)],
    16 => CHARACTERS_BASE16,
    _ => {
      unreachable!()
    }
  };

  let result = to_string(hash, base, chars).unwrap();

  // Left-pad with the zero character to ensure the output is at least 21 chars
  // (MAX_HASH_SIZE). `base_encode::to_string` produces variable-length output
  // depending on the numeric magnitude, which can cause panics when callers
  // slice the result at a fixed offset (e.g. `hash[..placeholder.len()]`).
  let pad_len = 21usize.saturating_sub(result.len());
  if pad_len > 0 {
    let mut padded = String::with_capacity(21);
    padded.extend(std::iter::repeat_n(chars[0] as char, pad_len));
    padded.push_str(&result);
    padded
  } else {
    result
  }
}

#[test]
fn test_xxhash_with_base() {
  assert_eq!(&xxhash_with_base(b"hello", 64), "YOFJeqs95x38-Gwetwem1");
  assert_eq!(&xxhash_with_base(b"hello", 36), "bpwli5k6mqm0gij09mxrh9npj");
  assert_eq!(&xxhash_with_base(b"hello", 16), "1838525eaacf79c77f3e1b07adc1e9b5");
}

#[test]
fn test_encode_hash_with_base_padding() {
  // 16-byte input with leading zeros: base_encode produces only 16 chars,
  // verify it gets left-padded to 21.
  let input = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1u8];
  let raw = to_string(&input, 64, CHARACTERS_BASE64).unwrap();
  assert_eq!(raw.len(), 16);

  let hash = encode_hash_with_base(&input, 64);
  assert_eq!(hash.len(), 21);
}
