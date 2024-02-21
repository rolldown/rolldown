use super::BuildErrorLike;

#[derive(Debug)]
pub struct SourceMapError {
  pub reason: String,
}

impl BuildErrorLike for SourceMapError {
  fn code(&self) -> &'static str {
    "SOURCEMAP_ERROR"
  }

  fn message(&self) -> String {
    format!("Error when using sourcemap for reporting an error: {}", self.reason)
  }
}
