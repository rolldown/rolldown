use crate::CowStr;

use super::MagicString;

impl<'text> MagicString<'text> {
  pub fn prepend(&mut self, source: impl Into<CowStr<'text>>) -> &mut Self {
    self.prepend_intro(source.into());
    self
  }

  pub fn prepend_left(&mut self, text_index: u32, content: impl Into<CowStr<'text>>) -> &mut Self {
    // Note: by_end_mut only errors when splitting an already-edited chunk,
    // but prepend operations don't require splitting edited chunks in practice.
    // We use expect here as this is an internal invariant.
    match self.by_end_mut(text_index).expect("prepend_left: unexpected split error") {
      Some(chunk) => chunk.prepend_outro(content.into()),
      None => self.prepend_intro(content.into()),
    }
    self
  }

  pub fn prepend_right(&mut self, text_index: u32, content: impl Into<CowStr<'text>>) -> &mut Self {
    // Note: by_start_mut only errors when splitting an already-edited chunk,
    // but prepend operations don't require splitting edited chunks in practice.
    // We use expect here as this is an internal invariant.
    match self.by_start_mut(text_index).expect("prepend_right: unexpected split error") {
      Some(chunk) => {
        chunk.prepend_intro(content.into());
      }
      None => self.prepend_outro(content.into()),
    }
    self
  }
}
