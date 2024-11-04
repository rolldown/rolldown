use std::sync::Arc;

use crate::{
  source_map::{locator::Locator, sourcemap_builder::SourcemapBuilder},
  MagicString,
};

#[derive(Debug)]
pub struct SourceMapOptions {
  pub include_content: bool,
  pub source: Arc<str>,
  pub hires: bool,
}

impl Default for SourceMapOptions {
  fn default() -> Self {
    Self { include_content: false, source: "".into(), hires: false }
  }
}

impl<'s> MagicString<'s> {
  pub fn source_map(&self, opts: SourceMapOptions) -> oxc_sourcemap::SourceMap {
    let mut source_builder = SourcemapBuilder::new(opts.hires);

    source_builder.set_source_and_content(&opts.source, &self.source);

    let locator = Locator::new(&self.source);

    self.intro.iter().for_each(|frag| {
      source_builder.advance(frag);
    });

    self.iter_chunks().for_each(|chunk| {
      chunk.intro.iter().for_each(|frag| {
        source_builder.advance(frag);
      });

      let name = if chunk.keep_in_mappings && chunk.is_edited() {
        Some(chunk.span.text(&self.source))
      } else {
        None
      };

      source_builder.add_chunk(chunk, &locator, &self.source, name);

      chunk.outro.iter().for_each(|frag| {
        source_builder.advance(frag);
      });
    });

    source_builder.into_source_map()
  }
}
