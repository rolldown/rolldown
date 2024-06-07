use oxc::span::Span;

pub trait SpanExt {
  fn is_empty(&self) -> bool;
}

impl SpanExt for Span {
  fn is_empty(&self) -> bool {
    self.start == 0 && self.end == 0
  }
}
