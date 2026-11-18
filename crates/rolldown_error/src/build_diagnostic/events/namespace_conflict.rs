use super::BuildEvent;
use arcstr::ArcStr;
use oxc::span::Span;

use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug)]
pub struct NamespaceConflictExporter {
  pub source: ArcStr,
  pub module_id: String,
  pub stable_id: String,
  pub span_of_identifier: Span,
}

#[derive(Debug)]
pub struct NamespaceConflict {
  pub binding: String,
  pub reexporting_module_id: String,
  pub reexporting_module_stable_id: String,
  pub exporters: Vec<NamespaceConflictExporter>,
}

impl BuildEvent for NamespaceConflict {
  fn kind(&self) -> EventKind {
    EventKind::NamespaceConflict
  }

  fn id(&self) -> Option<String> {
    Some(self.reexporting_module_id.clone())
  }

  fn exporter(&self) -> Option<String> {
    Some(self.reexporting_module_id.clone())
  }

  fn ids(&self) -> Option<Vec<String>> {
    Some(self.exporters.iter().map(|v| v.module_id.clone()).collect())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    let mut exporters = self.exporters.iter().map(|v| format!(r#""{0}""#, v.stable_id));

    let last = exporters.next_back().unwrap();

    format!(
      r#"Conflicting namespaces: "{}" re-exports "{}" from one of the modules {} and {} (will be ignored)."#,
      self.reexporting_module_stable_id,
      self.binding,
      exporters.collect::<Vec<_>>().join(", "),
      last
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {
    self.exporters.iter().for_each(|exporter| {
      let file_id = diagnostic.add_file(exporter.stable_id.clone(), exporter.source.clone());
      diagnostic.add_label(
        &file_id,
        exporter.span_of_identifier.start..exporter.span_of_identifier.end,
        "One matching export is here.".to_owned(),
      );
    });
  }
}
