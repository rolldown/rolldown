// Copied from https://github.com/rollup/rollup/blob/080d2245ab6b6298229ebe7258c2b96816e7c52d/rust/xxhash/src/lib.rs

use xxhash_rust::xxh3::xxh3_128;

use crate::base64::to_url_safe_base64;

pub fn xxhash_base64_url(input: &[u8]) -> String {
  let hash = xxh3_128(input).to_le_bytes();
  to_url_safe_base64(hash)
}
