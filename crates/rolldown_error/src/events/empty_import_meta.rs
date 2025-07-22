use crate::DiagnosticOptions;
use crate::diagnostic::Diagnostic;
use crate::events::BuildEvent;
use arcstr::ArcStr;
use oxc::span::Span;

#[derive(Debug)]
pub struct EmptyImportMeta {
  pub filename: String,
  pub source: ArcStr,
  pub span: Span,
  pub format: ArcStr,
}

impl BuildEvent for EmptyImportMeta {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::EmptyImportMeta
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "`import.meta` is not available with the `{}` output format and will be empty.",
      self.format
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);
    let file_id = diagnostic.add_file(filename, self.source.clone());

    diagnostic.title = format!(
      "`import.meta` is not available with the `{}` output format and will be empty.",
      self.format
    );

    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      String::from(
        "You need to set the output format to `esm` for `import.meta` to work correctly.",
      ),
    );
  }
}
