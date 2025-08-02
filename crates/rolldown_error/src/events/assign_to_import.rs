use arcstr::ArcStr;
use heck::ToUpperCamelCase;
use oxc::span::{CompactStr, Span};

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct AssignToImport {
  pub filename: ArcStr,
  pub source: ArcStr,
  pub span: Span,
  pub name: CompactStr,
}

impl BuildEvent for AssignToImport {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::AssignToImportError
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Cannot assign to import '{}'", self.name)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&*self.filename);

    let file_id = diagnostic.add_file(filename, self.source.clone());
    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      format!("Imports are immutable in JavaScript. To modify the value of this import, you must export a setter function in the imported file (e.g. 'set{}') and then import and call that function here instead.", self.name.to_upper_camel_case())
    );
  }
}
