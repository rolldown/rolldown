use crate::CowStr;

use super::MagicString;

impl<'text> MagicString<'text> {
  pub fn append(&mut self, source: impl Into<CowStr<'text>>) -> &mut Self {
    self.append_outro(source.into());
    self
  }

  /// # Errors
  /// Returns `Err` if `text_index` falls inside a chunk that has already been edited, since the
  /// chunk cannot be split there.
  ///
  /// # Example
  ///```rust
  /// use string_wizard::MagicString;
  /// let mut s = MagicString::new("01234");
  /// s.append_left(2, "a").unwrap();
  /// s.append_left(2, "b").unwrap();
  /// assert_eq!(s.to_string(), "01ab234")
  ///```
  pub fn append_left(
    &mut self,
    text_index: u32,
    content: impl Into<CowStr<'text>>,
  ) -> Result<&mut Self, String> {
    match self.by_end_mut(text_index)? {
      Some(chunk) => {
        chunk.append_outro(content.into());
      }
      None => self.append_intro(content.into()),
    }
    Ok(self)
  }

  /// # Errors
  /// Returns `Err` if `text_index` falls inside a chunk that has already been edited, since the
  /// chunk cannot be split there.
  ///
  /// # Example
  /// ```rust
  /// use string_wizard::MagicString;
  /// let mut s = MagicString::new("01234");
  /// s.append_right(2, "A").unwrap();
  /// s.append_right(2, "B").unwrap();
  /// s.append_left(2, "a").unwrap();
  /// s.append_left(2, "b").unwrap();
  /// assert_eq!(s.to_string(), "01abAB234")
  ///```
  pub fn append_right(
    &mut self,
    text_index: u32,
    content: impl Into<CowStr<'text>>,
  ) -> Result<&mut Self, String> {
    match self.by_start_mut(text_index)? {
      Some(chunk) => {
        chunk.append_intro(content.into());
      }
      None => self.append_outro(content.into()),
    }
    Ok(self)
  }
}
