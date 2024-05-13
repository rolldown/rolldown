use sugar_path::SugarPath;

pub trait PathExt {
  fn expect_to_str(&self) -> &str;

  fn expect_to_slash(&self) -> String;
}

impl PathExt for std::path::Path {
  fn expect_to_str(&self) -> &str {
    self.to_str().unwrap_or_else(|| {
      panic!("Failed to convert {:?} to valid utf8 str", self.display());
    })
  }

  fn expect_to_slash(&self) -> String {
    self
      .to_slash()
      .unwrap_or_else(|| panic!("Failed to convert {:?} to slash str", self.display()))
      .into_owned()
  }
}
