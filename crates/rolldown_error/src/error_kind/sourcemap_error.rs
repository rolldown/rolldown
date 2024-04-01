use super::BuildErrorLike;

#[derive(Debug)]
pub struct SourceMapError {
  pub error: oxc::sourcemap::Error,
}

impl BuildErrorLike for SourceMapError {
  fn code(&self) -> &'static str {
    "SOURCEMAP_ERROR"
  }

  fn message(&self) -> String {
    format!("Error when using sourcemap for reporting an error: {:?}", self.error)
  }
}
