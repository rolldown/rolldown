use std::path::PathBuf;

pub trait PathBufExt {
  fn expect_into_string(self) -> String;
}

impl PathBufExt for std::path::PathBuf {
  fn expect_into_string(self) -> String {
    self.into_os_string().into_string().unwrap_or_else(|input| {
      panic!("Failed to convert {:?} to valid utf8 string", PathBuf::from(input).display());
    })
  }
}
