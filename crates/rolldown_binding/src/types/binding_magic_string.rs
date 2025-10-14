#![expect(clippy::inherent_to_string)]
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
  pub fn replace(&mut self, from: String, to: String) {
    self.inner.replace(&from, to);
  }

  #[napi]
  pub fn replace_all(&mut self, from: String, to: String) {
    self.inner.replace_all(&from, to);
  }

  #[napi]
  pub fn prepend(&mut self, content: String) {
    self.inner.prepend(content);
  }

  #[napi]
  pub fn append(&mut self, content: String) {
    self.inner.append(content);
  }

  #[napi]
  pub fn prepend_left(&mut self, index: u32, content: String) {
    let byte_index =
      self.char_to_byte_mapper.char_to_byte(index as usize).expect("Invalid character index");
    self.inner.prepend_left(byte_index, content);
  }

  #[napi]
  pub fn prepend_right(&mut self, index: u32, content: String) {
    let byte_index =
      self.char_to_byte_mapper.char_to_byte(index as usize).expect("Invalid character index");
    self.inner.prepend_right(byte_index, content);
  }

  #[napi]
  pub fn append_left(&mut self, index: u32, content: String) {
    let byte_index =
      self.char_to_byte_mapper.char_to_byte(index as usize).expect("Invalid character index");
    self.inner.append_left(byte_index, content);
  }

  #[napi]
  pub fn append_right(&mut self, index: u32, content: String) {
    let byte_index =
      self.char_to_byte_mapper.char_to_byte(index as usize).expect("Invalid character index");
    self.inner.append_right(byte_index, content);
  }

  #[napi]
  pub fn overwrite(&mut self, start: u32, end: u32, content: String) {
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
  }

  #[napi]
  // TODO: claude code - Cannot change to &str: generates new String from MagicString internal representation
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
}
