use arcstr::ArcStr;
use oxc::span::Span;

use crate::{
  build_diagnostic::diagnostic::Diagnostic,
  types::{diagnostic_options::DiagnosticOptions, event_kind::EventKind},
};

use super::BuildEvent;

/// A used import whose binding depends on a module that `experimental.lazyBarrel` did not load.
#[derive(Debug)]
pub struct LazyBarrelUnloadedBinding {
  pub importer: String,
  pub stable_importer: String,
  pub importer_source: ArcStr,
  pub module_request: String,
  pub imported_specifier: String,
  pub imported_specifier_span: Span,
}

impl LazyBarrelUnloadedBinding {
  fn title(&self) -> String {
    format!(
      r#"Cannot fully resolve "{}" imported from "{}", because `experimental.lazyBarrel` did not load a module it may come from."#,
      self.imported_specifier, self.module_request
    )
  }
}

impl BuildEvent for LazyBarrelUnloadedBinding {
  fn kind(&self) -> EventKind {
    EventKind::LazyBarrelUnloadedBindingError
  }

  fn id(&self) -> Option<String> {
    Some(self.importer.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(r#"{} Imported by "{}"."#, self.title(), self.stable_importer)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {
    let file_id = diagnostic.add_file(&self.stable_importer, &self.importer_source);
    diagnostic.title = self.title();
    diagnostic.add_label(
      &file_id,
      self.imported_specifier_span.start..self.imported_specifier_span.end,
      String::from("This import is used"),
    );
    diagnostic.add_note(
      "This is a Rolldown bug. Please report it at https://github.com/rolldown/rolldown/issues. Set `experimental.lazyBarrel: false` to work around it.".to_string(),
    );
  }
}
