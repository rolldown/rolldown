#![expect(clippy::inherent_to_string)]
use napi::bindgen_prelude::This;
use napi_derive::napi;
use string_wizard::MagicString;

struct CharToByteMapper {
  char_to_byte: Vec<usize>,
}

impl CharToByteMapper {
  fn new(s: &str) -> Self {
    let mut char_to_byte = Vec::with_capacity(s.chars().count() + 1);
    char_to_byte.push(0); // char 0 is at byte 0

    let mut byte_offset = 0;
    for ch in s.chars() {
      byte_offset += ch.len_utf16();
      char_to_byte.push(byte_offset);
    }

    Self { char_to_byte }
  }

  fn char_to_byte(&self, char_offset: usize) -> Option<usize> {
    self.char_to_byte.get(char_offset).copied()
  }
}

#[napi]
pub struct BindingMagicString<'a> {
  pub(crate) inner: MagicString<'a>,
  char_to_byte_mapper: CharToByteMapper,
}

#[napi]
impl BindingMagicString<'_> {
  #[napi(constructor)]
  pub fn new(source: String) -> Self {
    let char_to_byte_mapper = CharToByteMapper::new(&source);
    Self { inner: MagicString::new(source), char_to_byte_mapper }
  }

  #[napi]
  pub fn replace<'s>(&'s mut self, this: This<'s>, from: String, to: String) -> This<'s> {
    self.inner.replace(&from, to);
    this
  }

  #[napi]
  pub fn replace_all<'s>(&'s mut self, this: This<'s>, from: String, to: String) -> This<'s> {
    self.inner.replace_all(&from, to);
    this
  }

  #[napi]
  pub fn prepend<'s>(&'s mut self, this: This<'s>, content: String) -> This<'s> {
    self.inner.prepend(content);
    this
  }

  #[napi]
  pub fn append<'s>(&'s mut self, this: This<'s>, content: String) -> This<'s> {
    self.inner.append(content);
    this
  }

  #[napi]
  pub fn prepend_left<'s>(&'s mut self, this: This<'s>, index: u32, content: String) -> This<'s> {
    let byte_index =
      self.char_to_byte_mapper.char_to_byte(index as usize).expect("Invalid character index");
    self.inner.prepend_left(byte_index, content);
    this
  }

  #[napi]
  pub fn prepend_right<'s>(&'s mut self, this: This<'s>, index: u32, content: String) -> This<'s> {
    let byte_index =
      self.char_to_byte_mapper.char_to_byte(index as usize).expect("Invalid character index");
    self.inner.prepend_right(byte_index, content);
    this
  }

  #[napi]
  pub fn append_left<'s>(&'s mut self, this: This<'s>, index: u32, content: String) -> This<'s> {
    let byte_index =
      self.char_to_byte_mapper.char_to_byte(index as usize).expect("Invalid character index");
    self.inner.append_left(byte_index, content);
    this
  }

  #[napi]
  pub fn append_right<'s>(&'s mut self, this: This<'s>, index: u32, content: String) -> This<'s> {
    let byte_index =
      self.char_to_byte_mapper.char_to_byte(index as usize).expect("Invalid character index");
    self.inner.append_right(byte_index, content);
    this
  }

  #[napi]
  pub fn overwrite<'s>(
    &'s mut self,
    this: This<'s>,
    start: u32,
    end: u32,
    content: String,
  ) -> This<'s> {
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start as usize).expect("Invalid start character index");
    let end_byte =
      self.char_to_byte_mapper.char_to_byte(end as usize).expect("Invalid end character index");
    self.inner.update_with(
      start_byte,
      end_byte,
      content,
      string_wizard::UpdateOptions { overwrite: true, keep_original: false },
    );
    this
  }

  #[napi]
  // TODO: should use `&str` instead. (claude code) Attempt failed due to generates new String from MagicString internal representation
  pub fn to_string(&self) -> String {
    self.inner.to_string()
  }

  #[napi]
  pub fn has_changed(&self) -> bool {
    self.inner.has_changed()
  }

  #[napi]
  pub fn length(&self) -> u32 {
    #[expect(clippy::cast_possible_truncation)]
    {
      self.inner.len() as u32
    }
  }

  #[napi]
  pub fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }

  #[napi]
  pub fn remove<'s>(&'s mut self, this: This<'s>, start: u32, end: u32) -> This<'s> {
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start as usize).expect("Invalid start character index");
    let end_byte =
      self.char_to_byte_mapper.char_to_byte(end as usize).expect("Invalid end character index");
    self.inner.remove(start_byte, end_byte);
    this
  }

  #[napi]
  pub fn update<'s>(
    &'s mut self,
    this: This<'s>,
    start: u32,
    end: u32,
    content: String,
  ) -> This<'s> {
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start as usize).expect("Invalid start character index");
    let end_byte =
      self.char_to_byte_mapper.char_to_byte(end as usize).expect("Invalid end character index");
    self.inner.update(start_byte, end_byte, content);
    this
  }

  #[napi]
  pub fn relocate<'s>(&'s mut self, this: This<'s>, start: u32, end: u32, to: u32) -> This<'s> {
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start as usize).expect("Invalid start character index");
    let end_byte =
      self.char_to_byte_mapper.char_to_byte(end as usize).expect("Invalid end character index");
    let to_byte =
      self.char_to_byte_mapper.char_to_byte(to as usize).expect("Invalid to character index");
    self.inner.relocate(start_byte, end_byte, to_byte);
    this
  }

  /// Alias for `relocate` to match the original magic-string API.
  /// Moves the characters from `start` to `end` to `index`.
  /// Returns `this` for method chaining.
  #[napi(js_name = "move")]
  pub fn move_<'s>(&'s mut self, this: This<'s>, start: u32, end: u32, index: u32) -> This<'s> {
    self.relocate(this, start, end, index)
  }

  #[napi]
  pub fn indent<'s>(&'s mut self, this: This<'s>, indentor: Option<String>) -> This<'s> {
    if let Some(indentor) = indentor {
      self
        .inner
        .indent_with(string_wizard::IndentOptions { indentor: Some(&indentor), exclude: &[] });
    } else {
      self.inner.indent();
    }
    this
  }

  /// Trims whitespace or specified characters from the start and end.
  #[napi]
  pub fn trim<'s>(&'s mut self, this: This<'s>, char_type: Option<String>) -> This<'s> {
    self.inner.trim(char_type.as_deref());
    this
  }

  /// Trims whitespace or specified characters from the start.
  #[napi]
  pub fn trim_start<'s>(&'s mut self, this: This<'s>, char_type: Option<String>) -> This<'s> {
    self.inner.trim_start(char_type.as_deref());
    this
  }

  /// Trims whitespace or specified characters from the end.
  #[napi]
  pub fn trim_end<'s>(&'s mut self, this: This<'s>, char_type: Option<String>) -> This<'s> {
    self.inner.trim_end(char_type.as_deref());
    this
  }

  /// Trims newlines from the start and end.
  #[napi]
  pub fn trim_lines<'s>(&'s mut self, this: This<'s>) -> This<'s> {
    self.inner.trim_lines();
    this
  }

  /// Returns the content between the specified original character positions.
  /// Supports negative indices (counting from the end).
  #[napi]
  pub fn slice(&self, start: Option<i64>, end: Option<i64>) -> napi::Result<String> {
    let mut start = start.unwrap_or(0);

    // char_count: the vector has N+1 elements for N characters (stores byte offset after each char)
    #[expect(clippy::cast_possible_wrap)]
    let char_count = (self.char_to_byte_mapper.char_to_byte.len() - 1) as i64;

    // Default end to char_count (original string length in characters)
    let mut end = end.unwrap_or(char_count);

    // Handle negative indices (matching original magic-string behavior)
    if char_count > 0 {
      if start < 0 {
        start = ((start % char_count) + char_count) % char_count;
      }
      if end < 0 {
        end = ((end % char_count) + char_count) % char_count;
      }
    }

    // Convert character indices to byte indices
    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start as usize).unwrap_or(self.inner.source().len());

    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let end_byte =
      self.char_to_byte_mapper.char_to_byte(end as usize).unwrap_or(self.inner.source().len());

    self.inner.slice(start_byte, Some(end_byte)).map_err(napi::Error::from_reason)
  }
}
