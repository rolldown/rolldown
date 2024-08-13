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
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::UnloadableDependency
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
    diagnostic: &mut crate::diagnostic::Diagnostic,
    opts: &DiagnosticOptions,
  ) {
    if let Some(context) = &self.context {
      let importer_file = diagnostic.add_file(context.importer_id.clone(), context.source.clone());

      diagnostic.title = format!(r#"Could not load {}"#, self.resolved);

      diagnostic.add_label(
        &importer_file,
        context.importee_span.start..context.importee_span.end,
        self.reason.to_string(),
      );
    } else {
      diagnostic.title = self.message(opts);
    }
  }
}
