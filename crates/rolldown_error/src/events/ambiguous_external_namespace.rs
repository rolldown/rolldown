use super::BuildEvent;
use arcstr::ArcStr;
use oxc::span::Span;

use crate::{EventKind, diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

#[derive(Debug)]
pub struct AmbiguousExternalNamespaceModule {
  // Point to `import { [identifier] } from ...` or `export [identifier]`
  pub source: ArcStr,
  pub module_id: String,
  pub stable_id: String,
  pub span_of_identifier: Span,
}

#[derive(Debug)]
pub struct AmbiguousExternalNamespace {
  pub ambiguous_export_name: String,
  pub importee: String,
  pub importer: AmbiguousExternalNamespaceModule,
  pub exporter: Vec<AmbiguousExternalNamespaceModule>,
}

impl BuildEvent for AmbiguousExternalNamespace {
  fn kind(&self) -> EventKind {
    EventKind::AmbiguousExternalNamespaceError
  }

  fn id(&self) -> Option<String> {
    Some(self.importer.module_id.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    let mut exporter = self.exporter.iter().map(|v| format!(r#""{0}""#, v.stable_id));

    let last = exporter.next_back().unwrap();

    format!(
      r#""{}" re-exports "{}" from one of the modules {} and {} (will be ignored)."#,
      self.importee,
      self.ambiguous_export_name,
      exporter.collect::<Vec<_>>().join(", "),
      last
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {
    diagnostic.title = "Found ambiguous export.".to_string();

    let file_id =
      diagnostic.add_file(self.importer.stable_id.clone(), self.importer.source.clone());

    diagnostic.add_label(
      &file_id,
      self.importer.span_of_identifier.start..self.importer.span_of_identifier.end,
      format!(r#""{}" re-exports "{}""#, self.importee, self.ambiguous_export_name),
    );

    self.exporter.iter().for_each(|exporter| {
      let file_id = diagnostic.add_file(exporter.stable_id.clone(), exporter.source.clone());
      diagnostic.add_label(
        &file_id,
        exporter.span_of_identifier.start..exporter.span_of_identifier.end,
        "One matching export is here.".to_owned(),
      );
    });
  }
}
