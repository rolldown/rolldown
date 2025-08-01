use arcstr::ArcStr;
use oxc::span::{CompactStr, Span};

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct ImportIsUndefined {
  pub filename: CompactStr,
  pub source: ArcStr,
  pub span: Span,
  pub name: CompactStr,
  pub stable_importer: String,
}

impl BuildEvent for ImportIsUndefined {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::ImportIsUndefined
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Import `{}` will always be undefined because there is no matching export in '{}'",
      self.name, self.stable_importer
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(self.filename.as_str());

    let file_id = diagnostic.add_file(filename, self.source.clone());

    diagnostic.add_label(&file_id, self.span.start..self.span.end, String::new());
  }
}
