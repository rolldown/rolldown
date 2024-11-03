use crate::CowStr;

use super::MagicString;

impl<'text> MagicString<'text> {
  pub fn prepend(&mut self, source: impl Into<CowStr<'text>>) -> &mut Self {
    self.prepend_intro(source.into());
    self
  }

  pub fn prepend_left(
    &mut self,
    text_index: usize,
    content: impl Into<CowStr<'text>>,
  ) -> &mut Self {
    match self.by_end_mut(text_index) {
      Some(chunk) => chunk.prepend_outro(content.into()),
      None => self.prepend_intro(content.into()),
    }
    self
  }

  pub fn prepend_right(
    &mut self,
    text_index: usize,
    content: impl Into<CowStr<'text>>,
  ) -> &mut Self {
    match self.by_start_mut(text_index) {
      Some(chunk) => {
        chunk.prepend_intro(content.into());
      }
      None => self.prepend_outro(content.into()),
    }
    self
  }
}
