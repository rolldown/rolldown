use oxc::span::{Span, SPAN};

pub trait SpanExt {
  fn is_empty(&self) -> bool;

  fn is_valid(&self, source: &str) -> bool;
}

impl SpanExt for Span {
  fn is_empty(&self) -> bool {
    self.start == 0 && self.end == 0
  }

  #[expect(clippy::cast_possible_truncation)]
  fn is_valid(&self, source: &str) -> bool {
    // DUMMY span is invalid
    if self == &SPAN {
      return false;
    }
    let source_len = source.len() as u32;
    // Check if the span is out of bounds
    if self.start > source_len || self.end > source_len {
      return false;
    }
    // Check if the span is empty
    if self.start > self.end {
      return false;
    }

    true
  }
}
