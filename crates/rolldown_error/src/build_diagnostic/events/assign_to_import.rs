use arcstr::ArcStr;
use heck::ToUpperCamelCase;
use oxc::span::Span;

use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
};

use super::BuildEvent;

#[derive(Debug)]
pub struct AssignToImport {
  pub filename: ArcStr,
  pub source: ArcStr,
  pub span: Span,
  pub name: ArcStr,
  pub import_decl_span: Option<Span>,
  pub imported_name: Option<ArcStr>,
}

impl BuildEvent for AssignToImport {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::AssignToImportError
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

    if let Some(import_span) = self.import_decl_span {
      // Use imported_name if available (for namespace imports), otherwise use name
      let label_name = self.imported_name.as_ref().unwrap_or(&self.name);
      diagnostic.add_label(
        &file_id,
        import_span.start..import_span.end,
        format!("'{label_name}' is imported here"),
      );
    }
  }
}
