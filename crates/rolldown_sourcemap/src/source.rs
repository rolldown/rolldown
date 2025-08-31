use std::fmt::Debug;

use oxc_sourcemap::SourceMap;

pub trait Source {
  fn sourcemap(&self) -> Option<&SourceMap>;
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

#[derive(Debug)]
pub struct SourceMapSource {
  content: String,
  sourcemap: SourceMap,
  pre_computed_lines_count: Option<u32>,
}

impl SourceMapSource {
  pub fn new(content: String, sourcemap: SourceMap) -> Self {
    Self { content, sourcemap, pre_computed_lines_count: None }
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
  fn sourcemap(&self) -> Option<&SourceMap> {
    Some(&self.sourcemap)
  }

  fn content(&self) -> &str {
    &self.content
  }

  fn lines_count(&self) -> u32 {
    self.pre_computed_lines_count.unwrap_or_else(|| lines_count(&self.content))
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
  memchr::memmem::find_iter(str.as_bytes(), "\n").count() as u32
}

#[test]
fn test() {
  assert_eq!(lines_count("a\nb\nc"), 2);
  assert_eq!(lines_count("a\nb\nc\n"), 3);
  assert_eq!(lines_count("a"), 0);
}
