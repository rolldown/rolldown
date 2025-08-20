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

    source_builder.set_source_and_content(&opts.source, self.source);

    let locator = Locator::new(self.source);

    self.intro.iter().for_each(|frag| {
      source_builder.advance(frag);
    });

    let utf16_index_map =
      precompute_utf16_index_map(self.source, self.iter_chunks().map(|chunk| chunk.start()));

    self.iter_chunks().for_each(|chunk| {
      chunk.intro.iter().for_each(|frag| {
        source_builder.advance(frag);
      });
      let name =
        (chunk.keep_in_mappings && chunk.is_edited()).then(|| chunk.span.text(self.source));

      source_builder.add_chunk(chunk, utf16_index_map[&chunk.start()], &locator, self.source, name);

      chunk.outro.iter().for_each(|frag| {
        source_builder.advance(frag);
      });
    });

    source_builder.into_source_map()
  }
}

fn precompute_utf16_index_map(
  source: &str,
  byte_indices: impl Iterator<Item = usize>,
) -> FxHashMap<usize, usize> {
  let mut byte_indices: Vec<usize> = byte_indices.collect();
  byte_indices.sort();
  let mut index = 0;
  let mut index_utf16 = 0;
  let mut map: FxHashMap<usize, usize> = Default::default();
  for &i in &byte_indices {
    index_utf16 += source[index..i].chars().map(|c| c.len_utf16()).sum::<usize>();
    index = i;
    map.insert(i, index_utf16);
  }
  map
}
