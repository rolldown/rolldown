pub mod locator;
pub mod sourcemap_builder;

use rustc_hash::FxHashMap;

/// Precompute a mapping from byte indices to UTF-16 column offsets.
/// Used by both `MagicString::source_map` and `MagicStringChain::source_map`.
pub(crate) fn precompute_utf16_index_map(
  source: &str,
  byte_indices: impl Iterator<Item = u32>,
) -> FxHashMap<u32, u32> {
  // Indices may be unsorted (e.g. after relocate()), so sort is required.
  let mut byte_indices: Vec<u32> = byte_indices.collect();
  byte_indices.sort_unstable();
  byte_indices.dedup();
  let mut index: u32 = 0;
  let mut index_utf16: u32 = 0;
  let mut map: FxHashMap<u32, u32> =
    FxHashMap::with_capacity_and_hasher(byte_indices.len(), Default::default());
  for &i in &byte_indices {
    let slice = &source[index as usize..i as usize];
    index_utf16 += if slice.is_ascii() {
      slice.len() as u32
    } else {
      slice.chars().map(|c| c.len_utf16() as u32).sum::<u32>()
    };
    index = i;
    map.insert(i, index_utf16);
  }
  map
}
