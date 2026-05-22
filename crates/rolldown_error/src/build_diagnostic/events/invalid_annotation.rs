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

    if self.is_before_function_declaration() {
      diagnostic.add_help(String::from(
        "If you intended to mark all calls of this function as side-effect-free, use `/* @__NO_SIDE_EFFECTS__ */` before the function declaration.",
      ));
    }

    diagnostic.add_help(String::from("For more information on how to use pure annotations correctly, check the documentation: https://rolldown.rs/in-depth/dead-code-elimination#pure"));
  }
}

impl InvalidAnnotation {
  fn is_before_function_declaration(&self) -> bool {
    fn is_ident_continue(c: char) -> bool {
      c == '_' || c == '$' || c.is_ascii_alphanumeric()
    }

    fn consume_keyword<'a>(mut text: &'a str, keyword: &str) -> Option<&'a str> {
      text = text.strip_prefix(keyword)?;
      if text.chars().next().is_none_or(|c| !is_ident_continue(c)) { Some(text) } else { None }
    }

    let mut rest = self.source[self.span.end as usize..].trim_start();

    if let Some(next) = consume_keyword(rest, "export") {
      rest = next.trim_start();
      if let Some(next) = consume_keyword(rest, "default") {
        rest = next.trim_start();
      }
    }

    if let Some(next) = consume_keyword(rest, "async") {
      rest = next.trim_start();
    }

    consume_keyword(rest, "function").is_some()
  }
}
