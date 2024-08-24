use rolldown_sourcemap::{ConcatSource, RawSource};

pub mod app;
pub mod cjs;
pub mod esm;
pub mod utils;

pub trait AppendRawString {
  fn append_raw_string(&mut self, s: String);
  fn append_optional_raw_string(&mut self, s: Option<String>);
}

impl AppendRawString for ConcatSource {
  fn append_raw_string(&mut self, s: String) {
    self.add_source(Box::new(RawSource::new(s)));
  }

  fn append_optional_raw_string(&mut self, s: Option<String>) {
    if let Some(s) = s {
      self.append_raw_string(s);
    }
  }
}
