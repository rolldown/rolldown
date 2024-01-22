use super::BuildErrorLike;
use crate::diagnostic::{DiagnosticBuilder, SpanLabel};
use ariadne::Label;

#[derive(Debug)]
pub struct InvalidGlob {
  pub(crate) pattern: Option<String>,
  pub(crate) error: String,
  pub(crate) locations: Option<Vec<SpanLabel>>,
}

impl From<wax::BuildError> for InvalidGlob {
  fn from(value: wax::BuildError) -> Self {
    Self {
      pattern: None,
      error: value.to_string(),
      locations: Some(
        value
          .locations()
          .map(|loc| {
            let span = loc.span();
            Label::new(("Invalid".to_owned(), span.0..span.1))
          })
          .collect::<Vec<_>>(),
      ),
    }
  }
}

impl InvalidGlob {
  pub fn new(pattern: String, error: String) -> Self {
    Self { pattern: Some(pattern), error, locations: None }
  }
}

impl BuildErrorLike for InvalidGlob {
  fn code(&self) -> &'static str {
    "INVALID_GLOB"
  }

  fn message(&self) -> String {
    self.pattern.as_ref().map_or_else(
      || format!("Invalid globset: {}", self.error),
      |pattern| format!("Invalid glob pattern `{pattern}`: {}", self.error),
    )
  }

  fn diagnostic_builder(&self) -> crate::diagnostic::DiagnosticBuilder {
    DiagnosticBuilder {
      code: Some(self.code()),
      summary: Some(self.error.clone()),
      labels: self.locations.clone(),
      ..Default::default()
    }
  }
}
