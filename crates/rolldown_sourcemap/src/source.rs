use std::fmt::Debug;

use crate::SourceMap;

pub trait Source {
  fn sourcemap(&self) -> Option<&SourceMap>;
  /// Move this source's sourcemap out when it is exclusively owned.
  ///
  /// Borrowed sources keep the default `None`; `SourceJoiner` then falls back
  /// to `sourcemap` and copies only those borrowed entries into its owned result.
  fn take_sourcemap(&mut self) -> Option<SourceMap> {
    None
  }
  fn content(&self) -> &str;
  fn lines_count(&self) -> u32 {
    lines_count(self.content())
  }
}

impl Source for &str {
  fn sourcemap(&self) -> Option<&SourceMap> {
    None
  }

  fn content(&self) -> &str {
    self
  }
}

impl Source for String {
  fn sourcemap(&self) -> Option<&SourceMap> {
    None
  }

  fn content(&self) -> &str {
    self
  }
}

/// Code paired with an exclusively owned map. Content may be owned or shared separately.
#[derive(Debug)]
pub struct SourceMapSource<T = String> {
  content: T,
  sourcemap: Option<SourceMap>,
  pre_computed_lines_count: Option<u32>,
}

impl<T: AsRef<str>> SourceMapSource<T> {
  pub fn new(content: T, sourcemap: SourceMap) -> Self {
    Self { content, sourcemap: Some(sourcemap), pre_computed_lines_count: None }
  }

  #[must_use]
  pub fn with_pre_compute_sourcemap_data(mut self, pre_compute: bool) -> Self {
    if pre_compute {
      self.pre_computed_lines_count = Some(lines_count(self.content.as_ref()));
    }
    self
  }
}

impl<T: AsRef<str>> Source for SourceMapSource<T> {
  fn sourcemap(&self) -> Option<&SourceMap> {
    self.sourcemap.as_ref()
  }

  fn content(&self) -> &str {
    self.content.as_ref()
  }

  fn lines_count(&self) -> u32 {
    self.pre_computed_lines_count.unwrap_or_else(|| lines_count(self.content.as_ref()))
  }

  fn take_sourcemap(&mut self) -> Option<SourceMap> {
    self.sourcemap.take()
  }
}

impl Source for &Box<dyn Source + Send + Sync> {
  fn sourcemap(&self) -> Option<&SourceMap> {
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

#[test]
fn mapped_source_can_borrow_code_and_yield_its_map() {
  let code = "first();\nsecond();";
  let mut source =
    SourceMapSource::new(code, SourceMap::default()).with_pre_compute_sourcemap_data(true);
  assert_eq!(source.content(), code);
  assert_eq!(source.lines_count(), 1);
  assert!(source.take_sourcemap().is_some());
  assert!(source.take_sourcemap().is_none());
  assert!(source.sourcemap().is_none());
  assert_eq!(source.content(), code);
  assert_eq!(source.lines_count(), 1);
}
