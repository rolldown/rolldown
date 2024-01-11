use super::BuildErrorLike;
use crate::diagnostic::DiagnosticBuilder;
use ariadne::Label;

#[derive(Debug)]
pub struct InvalidGlob {
  pub(crate) pattern: Option<String>,
  pub(crate) error: wax::BuildError,
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
      summary: Some(self.error.to_string()),
      labels: Some(
        self
          .error
          .locations()
          .map(|loc| {
            let span = loc.span();
            Label::new(("Invalid".to_owned(), span.0..span.1))
          })
          .collect::<Vec<_>>(),
      ),
      ..Default::default()
    }
  }
}
