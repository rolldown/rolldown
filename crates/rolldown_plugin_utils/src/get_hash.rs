use rolldown_utils::xxhash::xxhash_with_base;

pub fn get_hash(input: &str) -> String {
  let mut hash = xxhash_with_base(input.as_bytes(), 16);
  hash.truncate(8);
  hash
}
