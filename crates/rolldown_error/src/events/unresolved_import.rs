use crate::types::diagnostic_options::DiagnosticOptions;
use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;

#[derive(Debug)]
pub struct UnresolvedImportImporter {
  pub id: ArcStr,
  pub span: Span,
  pub source: ArcStr,
}

#[derive(Debug)]
pub struct UnresolvedImport {
  pub(crate) reason: ArcStr,
  pub(crate) resolved: ArcStr,
  pub(crate) importer: Option<UnresolvedImportImporter>,
}

impl BuildEvent for UnresolvedImport {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::UnresolvedImport
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Could not resolve {}{} - {}.",
      self.resolved,
      self.importer.as_ref().map(|i| format!(" (imported by {})", i.id)).unwrap_or_default(),
      self.reason
    )
  }

  fn on_diagnostic(
    &self,
    diagnostic: &mut crate::diagnostic::Diagnostic,
    opts: &DiagnosticOptions,
  ) {
    if let Some(importer) = &self.importer {
      let importer_file = diagnostic.add_file(importer.id.clone(), importer.source.clone());

      diagnostic.title = format!(r#"Could not resolve {}"#, self.resolved);

      diagnostic.add_label(
        &importer_file,
        importer.span.start..importer.span.end,
        self.reason.to_string(),
      );
    } else {
      diagnostic.title = self.message(opts);
    }
  }
}
