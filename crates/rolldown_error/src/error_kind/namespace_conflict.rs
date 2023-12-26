use std::sync::Arc;

use ariadne::Label;
use oxc::span::Span;

use crate::diagnostic::DiagnosticBuilder;

use super::BuildErrorLike;

#[derive(Debug)]
pub struct NamespaceConflict {
  pub reexport_module: String,
  pub sources: Vec<String>,
  pub reexport_module_source: Arc<str>,
  pub symbol: String,
  pub symbol_span: Span,
}

impl BuildErrorLike for NamespaceConflict {
  fn code(&self) -> &'static str {
    "NAMESPACE_CONFLICT"
  }

  fn message(&self) -> String {
    format!(
      r#""{}" re-exports "{}" from one of the modules {} (will be ignored)."#,
      self.reexport_module,
      self.symbol,
      self.sources.iter().map(|v| format!(r#""{v}""#)).collect::<Vec<_>>().join(" and ")
    )
  }

  fn diagnostic_builder(&self) -> crate::diagnostic::DiagnosticBuilder {
    DiagnosticBuilder {
      code: Some(self.code()),
      summary: Some("Found ambiguous export.".to_string()),
      files: Some(vec![(
        self.reexport_module.to_string(),
        self.reexport_module_source.to_string(),
      )]),
      labels: Some(vec![Label::new((
        self.reexport_module.to_string(),
        (self.symbol_span.start as usize..self.symbol_span.end as usize),
      ))
      .with_message(self.message())]),
      ..Default::default()
    }
  }
}
