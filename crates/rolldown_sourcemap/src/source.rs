use std::fmt::Debug;

use crate::{PackedSourceMap, SourceMap};

pub trait Source {
  /// The sourcemap of this source. It is held packed: sources wait for their chunk to be
  /// joined, and `SourceJoiner` joins packed maps without decoding them.
  fn sourcemap(&self) -> Option<&PackedSourceMap>;
  /// Move this source's sourcemap out when it is exclusively owned.
  ///
  /// Borrowed sources keep the default `None`; `SourceJoiner` then falls back
  /// to `sourcemap` and copies only those borrowed entries into its owned result.
  fn take_sourcemap(&mut self) -> Option<PackedSourceMap> {
    None
  }
  fn content(&self) -> &str;
  fn lines_count(&self) -> u32 {
    lines_count(self.content())
  }
}

impl Source for &str {
  fn sourcemap(&self) -> Option<&PackedSourceMap> {
    None
  }

  fn content(&self) -> &str {
    self
  }
}

impl Source for String {
  fn sourcemap(&self) -> Option<&PackedSourceMap> {
    None
  }

  fn content(&self) -> &str {
    self
  }
}

#[derive(Debug)]
pub struct SourceMapSource {
  content: String,
  sourcemap: Option<PackedSourceMap>,
  pre_computed_lines_count: Option<u32>,
}

impl SourceMapSource {
  pub fn new(content: String, sourcemap: SourceMap) -> Self {
    Self::from_packed(content, sourcemap.into_packed())
  }

  pub fn from_packed(content: String, sourcemap: PackedSourceMap) -> Self {
    Self { content, sourcemap: Some(sourcemap), pre_computed_lines_count: None }
  }

  #[must_use]
  pub fn with_pre_compute_sourcemap_data(mut self, pre_compute: bool) -> Self {
    if pre_compute {
      self.pre_computed_lines_count = Some(lines_count(&self.content));
    }
    self
  }
}

impl Source for SourceMapSource {
  fn sourcemap(&self) -> Option<&PackedSourceMap> {
    self.sourcemap.as_ref()
  }

  fn content(&self) -> &str {
    &self.content
  }

  fn lines_count(&self) -> u32 {
    self.pre_computed_lines_count.unwrap_or_else(|| lines_count(&self.content))
  }

  fn take_sourcemap(&mut self) -> Option<PackedSourceMap> {
    self.sourcemap.take()
  }
}

impl Source for &Box<dyn Source + Send + Sync> {
  fn sourcemap(&self) -> Option<&PackedSourceMap> {
    self.as_ref().sourcemap()
  }

  fn content(&self) -> &str {
    self.as_ref().content()
  }

  fn lines_count(&self) -> u32 {
    self.as_ref().lines_count()
  }
}

#[expect(clippy::cast_possible_truncation)]
#[inline]
fn lines_count(str: &str) -> u32 {
  memchr::memchr_iter(b'\n', str.as_bytes()).count() as u32
}

#[test]
fn test() {
  assert_eq!(lines_count("a\nb\nc"), 2);
  assert_eq!(lines_count("a\nb\nc\n"), 3);
  assert_eq!(lines_count("a"), 0);
}
