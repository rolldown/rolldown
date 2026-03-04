use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::{
  MagicString,
  source_map::{
    locator::Locator,
    sourcemap_builder::{Hires, SourcemapBuilder},
  },
};

#[derive(Debug)]
pub struct SourceMapOptions {
  pub include_content: bool,
  pub source: Arc<str>,
  pub hires: Hires,
}

impl Default for SourceMapOptions {
  fn default() -> Self {
    Self { include_content: false, source: "".into(), hires: Hires::default() }
  }
}

impl MagicString<'_> {
  pub fn source_map(&self, opts: SourceMapOptions) -> oxc_sourcemap::SourceMap {
    let mut source_builder = SourcemapBuilder::new(opts.hires);

    source_builder.set_source_and_content(&opts.source, &self.source);

    let locator = Locator::new(&self.source);

    self.intro.iter().for_each(|frag| {
      source_builder.advance(frag);
    });

    let utf16_index_map =
      precompute_utf16_index_map(&self.source, self.iter_chunks().map(|chunk| chunk.start()));

    self.iter_chunks().for_each(|chunk| {
      chunk.intro.iter().for_each(|frag| {
        source_builder.advance(frag);
      });
      let name =
        (chunk.keep_in_mappings && chunk.is_edited()).then(|| chunk.span.text(&self.source));

      source_builder.add_chunk(
        chunk,
        utf16_index_map[&chunk.start()],
        &locator,
        &self.source,
        name,
      );

      chunk.outro.iter().for_each(|frag| {
        source_builder.advance(frag);
      });
    });

    source_builder.into_source_map()
  }
}

fn precompute_utf16_index_map(
  source: &str,
  byte_indices: impl Iterator<Item = u32>,
) -> FxHashMap<u32, u32> {
  // Chunk traversal order may not be sorted (e.g. after relocate()), so sort is required.
  let mut byte_indices: Vec<u32> = byte_indices.collect();
  byte_indices.sort_unstable();
  let mut index: u32 = 0;
  let mut index_utf16: u32 = 0;
  let mut map: FxHashMap<u32, u32> =
    FxHashMap::with_capacity_and_hasher(byte_indices.len(), Default::default());
  for &i in &byte_indices {
    let slice = &source[index as usize..i as usize];
    // Fast path: ASCII strings have 1:1 byte-to-UTF-16 mapping
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
