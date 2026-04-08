use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::{
  MagicString,
  source_map::{
    locator::Locator,
    precompute_utf16_index_map,
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
  pub fn source_map(&self, opts: SourceMapOptions) -> oxc_sourcemap::SourceMap<'static> {
    let mut source_builder = SourcemapBuilder::new(opts.hires);

    source_builder.set_source_and_content(&opts.source, &self.source);

    let locator = Locator::new(&self.source);

    // Seed `names` with every stored range up front, so the array reflects what was replaced
    // rather than whatever chunks happen to line up. A chunk only carries a name when its own
    // span is exactly one of those ranges — a range split across chunks stays in `names` with
    // no mapping pointing at it, which is what magic-string does.
    let name_ids: FxHashMap<&str, u32> = self
      .stored_names_ordered()
      .into_iter()
      .map(|(name, _)| (name, source_builder.add_name(name)))
      .collect();

    self.intro.iter().for_each(|frag| {
      source_builder.advance(frag);
    });

    let utf16_index_map =
      precompute_utf16_index_map(&self.source, self.iter_chunks().map(|chunk| chunk.start()));

    self.iter_chunks().for_each(|chunk| {
      chunk.intro.iter().for_each(|frag| {
        source_builder.advance(frag);
      });
      let name_id = (chunk.keep_in_mappings && chunk.is_edited())
        .then(|| name_ids.get(chunk.span.text(&self.source)).copied())
        .flatten();

      source_builder.add_chunk(
        chunk,
        utf16_index_map[&chunk.start()],
        &locator,
        &self.source,
        name_id,
      );

      chunk.outro.iter().for_each(|frag| {
        source_builder.advance(frag);
      });
    });

    let mut source_map = source_builder.into_source_map();

    if self.ignore_list {
      // The source is always at index 0 for a single MagicString instance.
      source_map.set_ignore_list(vec![0]);
    }

    source_map
  }
}
