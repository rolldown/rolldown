//! A small binary format for saved results. Decoding returns `None` on any unexpected byte, so
//! damaged data is treated as not found.

pub struct Encoder {
  buf: Vec<u8>,
}

impl Encoder {
  pub fn new() -> Self {
    Self { buf: Vec::with_capacity(1024) }
  }

  pub fn finish(self) -> Vec<u8> {
    self.buf
  }

  pub fn u8(&mut self, value: u8) {
    self.buf.push(value);
  }

  pub fn bool(&mut self, value: bool) {
    self.u8(u8::from(value));
  }

  pub fn u32(&mut self, value: u32) {
    self.buf.extend_from_slice(&value.to_le_bytes());
  }

  pub fn len(&mut self, value: usize) {
    self.u32(u32::try_from(value).expect("cache data should be smaller than 4 GiB"));
  }

  pub fn bytes(&mut self, value: &[u8]) {
    self.len(value.len());
    self.buf.extend_from_slice(value);
  }

  pub fn str(&mut self, value: &str) {
    self.bytes(value.as_bytes());
  }

  pub fn opt<T>(&mut self, value: Option<T>, write: impl FnOnce(&mut Self, T)) {
    match value {
      None => self.u8(0),
      Some(value) => {
        self.u8(1);
        write(self, value);
      }
    }
  }

  pub fn opt_str(&mut self, value: Option<&str>) {
    self.opt(value, Self::str);
  }

  pub fn seq<T>(
    &mut self,
    items: impl ExactSizeIterator<Item = T>,
    mut write: impl FnMut(&mut Self, T),
  ) {
    self.len(items.len());
    for item in items {
      write(self, item);
    }
  }
}

pub struct Decoder<'a> {
  buf: &'a [u8],
}

impl<'a> Decoder<'a> {
  pub fn new(buf: &'a [u8]) -> Self {
    Self { buf }
  }

  pub fn is_empty(&self) -> bool {
    self.buf.is_empty()
  }

  fn take(&mut self, n: usize) -> Option<&'a [u8]> {
    if self.buf.len() < n {
      return None;
    }
    let (head, rest) = self.buf.split_at(n);
    self.buf = rest;
    Some(head)
  }

  pub fn u8(&mut self) -> Option<u8> {
    self.take(1).map(|b| b[0])
  }

  pub fn bool(&mut self) -> Option<bool> {
    match self.u8()? {
      0 => Some(false),
      1 => Some(true),
      _ => None,
    }
  }

  pub fn u32(&mut self) -> Option<u32> {
    self.take(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
  }

  pub fn len(&mut self) -> Option<usize> {
    let len = self.u32()? as usize;
    // A length can never be larger than the bytes left. This also stops huge allocations.
    (len <= self.buf.len()).then_some(len)
  }

  pub fn bytes(&mut self) -> Option<&'a [u8]> {
    let len = self.len()?;
    self.take(len)
  }

  pub fn str(&mut self) -> Option<&'a str> {
    std::str::from_utf8(self.bytes()?).ok()
  }

  pub fn string(&mut self) -> Option<String> {
    self.str().map(ToOwned::to_owned)
  }

  /// The outer `None` means damaged data. The inner one is a value that was not set.
  #[expect(clippy::option_option)]
  pub fn opt<T>(&mut self, read: impl FnOnce(&mut Self) -> Option<T>) -> Option<Option<T>> {
    match self.u8()? {
      0 => Some(None),
      1 => read(self).map(Some),
      _ => None,
    }
  }

  #[expect(clippy::option_option)]
  pub fn opt_string(&mut self) -> Option<Option<String>> {
    self.opt(Self::string)
  }

  pub fn seq<T>(&mut self, mut read: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
    let len = self.u32()? as usize;
    // Every item takes at least one byte.
    if len > self.buf.len() {
      return None;
    }
    let mut items = Vec::with_capacity(len);
    for _ in 0..len {
      items.push(read(self)?);
    }
    Some(items)
  }
}
