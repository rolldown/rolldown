use crate::types::diagnostic_options::DiagnosticOptions;
use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;

#[derive(Debug)]
pub struct UnloadableDependencyContext {
  pub source: ArcStr,
  pub importer_id: ArcStr,
  pub importee_span: Span,
}

#[derive(Debug)]
pub struct UnloadableDependency {
  pub(crate) reason: ArcStr,
  pub(crate) resolved: ArcStr,
  pub(crate) context: Option<UnloadableDependencyContext>,
}

impl BuildEvent for UnloadableDependency {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::UnloadableDependencyError
  }

  fn id(&self) -> Option<String> {
    self.context.as_ref().map(|context| context.importer_id.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Could not load {}{} - {}.",
      self.resolved,
      self
        .context
        .as_ref()
        .map(|i| format!(" (imported by {})", i.importer_id))
        .unwrap_or_default(),
      self.reason
    )
  }

  fn on_diagnostic(
    &self,
    diagnostic: &mut crate::build_diagnostic::diagnostic::Diagnostic,
    opts: &DiagnosticOptions,
  ) {
    match &self.context {
      Some(context) => {
        let importer_file =
          diagnostic.add_file(context.importer_id.clone(), context.source.clone());

        diagnostic.title = format!("Could not load {}", self.resolved);

        diagnostic.add_label(
          &importer_file,
          context.importee_span.start..context.importee_span.end,
          self.reason.to_string(),
        );
      }
      _ => {
        diagnostic.title = self.message(opts);
      }
    }
  }
}
