use std::sync::Arc;

use ariadne::Label;
use oxc::span::Span;

use crate::diagnostic::DiagnosticBuilder;

use super::BuildErrorLike;

#[derive(Debug)]
pub struct MissingExport {
  pub importer: String,
  pub importee: String,
  pub importer_source: Arc<str>,
  pub symbol: String,
  pub symbol_span: Span,
}

impl BuildErrorLike for MissingExport {
  fn code(&self) -> &'static str {
    "MISSING_EXPORT"
  }

  fn message(&self) -> String {
    format!(
      r#""{}" is not exported by "{}", imported by "{}"."#,
      self.symbol, self.importee, self.importer
    )
  }

  fn diagnostic_builder(&self) -> crate::diagnostic::DiagnosticBuilder {
    DiagnosticBuilder {
      code: Some(self.code()),
      summary: Some("Found missing export.".to_string()),
      files: Some(vec![(self.importer.to_string(), self.importer_source.to_string())]),
      labels: Some(vec![Label::new((
        self.importer.to_string(),
        (self.symbol_span.start as usize..self.symbol_span.end as usize),
      ))
      .with_message(self.message())]),
      ..Default::default()
    }
  }
}
