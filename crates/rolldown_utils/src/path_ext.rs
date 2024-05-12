pub trait PathExt {
  fn expect_to_str(&self) -> &str;
}

impl PathExt for std::path::Path {
  fn expect_to_str(&self) -> &str {
    self.to_str().unwrap_or_else(|| {
      panic!("Failed to convert {:?} to valid utf8 str", self.display());
    })
  }
}
