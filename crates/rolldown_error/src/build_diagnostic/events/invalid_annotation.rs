use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug)]
pub struct InvalidAnnotation {
  pub module_id: String,
  /// Verbatim comment text, e.g. `/* #__PURE__ */` or `/* @__PURE__ */`.
  pub annotation: String,
  pub source: ArcStr,
  /// Span of the comment within the module source.
  pub span: Span,
}

impl BuildEvent for InvalidAnnotation {
  fn kind(&self) -> EventKind {
    EventKind::InvalidAnnotation
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "A comment \"{}\" in \"{}\" contains an annotation that Rolldown cannot interpret due to the position of the comment.",
      self.annotation,
      opts.stabilize_path(&self.module_id),
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.module_id);
    let file_id = diagnostic.add_file(filename, self.source.clone());

    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      String::from("comment ignored due to position"),
    );

    diagnostic.add_help(String::from("For more information on how to use pure annotations correctly, check the documentation: https://rolldown.rs/in-depth/dead-code-elimination#pure"));
  }
}
