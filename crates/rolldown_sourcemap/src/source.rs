use oxc::sourcemap::{ConcatSourceMapBuilder, SourceMap};

use crate::lines_count;

pub trait Source {
  fn sourcemap(&self) -> Option<&SourceMap>;
  fn content(&self) -> &str;
  fn lines_count(&self) -> u32;
  #[allow(clippy::wrong_self_convention)]
  fn join(
    &self,
    final_source: &mut String,
    sourcemap_builder: &mut Option<ConcatSourceMapBuilder>,
    line_offset: u32,
  );
}

impl<'a> Source for &'a str {
  fn sourcemap(&self) -> Option<&SourceMap> {
    None
  }

  fn content(&self) -> &str {
    self
  }

  fn lines_count(&self) -> u32 {
    lines_count(self)
  }

  fn join(
    &self,
    final_source: &mut String,
    _sourcemap_builder: &mut Option<ConcatSourceMapBuilder>,
    _line_offset: u32,
  ) {
    final_source.push_str(self);
  }
}

impl Source for String {
  fn sourcemap(&self) -> Option<&SourceMap> {
    None
  }

  fn content(&self) -> &str {
    self
  }

  fn lines_count(&self) -> u32 {
    lines_count(self)
  }

  fn join(
    &self,
    source: &mut String,
    _sourcemap_builder: &mut Option<ConcatSourceMapBuilder>,
    _line_offset: u32,
  ) {
    source.push_str(self);
  }
}

pub struct SourceMapSource {
  content: String,
  sourcemap: SourceMap,
  lines_count: u32,
}

impl SourceMapSource {
  pub fn new(content: String, sourcemap: SourceMap, lines_count: u32) -> Self {
    Self { content, sourcemap, lines_count }
  }
}

impl Source for SourceMapSource {
  fn sourcemap(&self) -> Option<&SourceMap> {
    Some(&self.sourcemap)
  }

  fn content(&self) -> &str {
    &self.content
  }

  fn lines_count(&self) -> u32 {
    self.lines_count
  }

  fn join(
    &self,
    source: &mut String,
    sourcemap_builder: &mut Option<ConcatSourceMapBuilder>,
    line_offset: u32,
  ) {
    if let Some(sourcemap_builder) = sourcemap_builder {
      sourcemap_builder.add_sourcemap(&self.sourcemap, line_offset);
    }

    source.push_str(&self.content);
  }
}

impl Source for Box<dyn Source + Send> {
  fn sourcemap(&self) -> Option<&SourceMap> {
    self.as_ref().sourcemap()
  }

  fn content(&self) -> &str {
    self.as_ref().content()
  }

  fn lines_count(&self) -> u32 {
    self.as_ref().lines_count()
  }

  fn join(
    &self,
    source: &mut String,
    sourcemap_builder: &mut Option<ConcatSourceMapBuilder>,
    line_offset: u32,
  ) {
    self.as_ref().join(source, sourcemap_builder, line_offset);
  }
}
