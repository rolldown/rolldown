// Copied from https://github.com/rollup/rollup/blob/080d2245ab6b6298229ebe7258c2b96816e7c52d/rust/xxhash/src/lib.rs

use base_encode::to_string;
use xxhash_rust::xxh3::xxh3_128;

use crate::base64::to_url_safe_base64;

const CHARACTERS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-_";

pub fn xxhash_base64_url(input: &[u8]) -> String {
  let hash = xxh3_128(input).to_le_bytes();
  to_url_safe_base64(hash)
}

pub fn xxhash_with_base(input: &[u8], base: u8) -> String {
  let hash = if input.len() == 16 { input } else { &xxh3_128(input).to_le_bytes() };

  to_string(hash, base, &CHARACTERS[..base as usize]).unwrap()
}
