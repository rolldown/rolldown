use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug)]
pub struct DroppedImportAttribute {
  pub module_id: String,
  pub source: ArcStr,
  pub span: Span,
  pub importee: String,
  pub format: ArcStr,
}

impl BuildEvent for DroppedImportAttribute {
  fn kind(&self) -> EventKind {
    EventKind::DroppedImportAttribute
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "The import attributes of \"{}\" are dropped in the `{}` output format.",
      self.importee, self.format
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let file_id = diagnostic.add_file(opts.stabilize_path(&self.module_id), self.source.clone());
    diagnostic.add_label(&file_id, self.span.start..self.span.end, "these are dropped".to_string());
  }
}
