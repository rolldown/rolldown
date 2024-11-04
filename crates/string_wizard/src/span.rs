#[derive(Debug, Default, Clone, Copy)]
pub struct Span(pub usize, pub usize);

impl Span {
  pub fn start(&self) -> usize {
    self.0
  }

  pub fn end(&self) -> usize {
    self.1
  }

  pub fn text<'s>(&self, source: &'s str) -> &'s str {
    // This crate doesn't support usize which is u16 on 16-bit platforms.
    // So, we can safely cast usize/u32 to usize.
    &source[self.start()..self.end()]
  }
}
