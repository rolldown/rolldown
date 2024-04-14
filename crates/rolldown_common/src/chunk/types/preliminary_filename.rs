use std::borrow::Cow;

#[derive(Debug)]
pub struct PreliminaryFilename {
  pub filename: String,
  pub hash_placeholder: Option<String>,
}

impl PreliminaryFilename {
  pub fn as_str(&self) -> &str {
    &self.filename
  }

  pub fn finalize(&self, hash: &str) -> Cow<str> {
    match &self.hash_placeholder {
      Some(placeholder) => self.filename.replace(placeholder, hash).into(),
      None => Cow::Borrowed(&self.filename),
    }
  }
}
