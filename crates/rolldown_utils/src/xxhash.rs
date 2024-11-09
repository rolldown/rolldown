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

pub fn xxhash_with_base(input: &[u8], base: u8) -> String {
  let hash = if input.len() == 16 { input } else { &xxh3_128(input).to_le_bytes() };

  let chars = match base {
    64 => CHARACTERS_BASE64,
    36 => &CHARACTERS_BASE64[26..(26 + 36)],
    16 => CHARACTERS_BASE16,
    _ => {
      unreachable!()
    }
  };

  to_string(hash, base, chars).unwrap()
}

#[test]
fn test_xxhash_with_base() {
  assert_eq!(&xxhash_with_base(b"hello", 64), "YOFJeqs95x38-Gwetwem1");
  assert_eq!(&xxhash_with_base(b"hello", 36), "bpwli5k6mqm0gij09mxrh9npj");
  assert_eq!(&xxhash_with_base(b"hello", 16), "1838525eaacf79c77f3e1b07adc1e9b5");
}
