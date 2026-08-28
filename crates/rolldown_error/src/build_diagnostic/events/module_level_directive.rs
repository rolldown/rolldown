use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug)]
pub struct ModuleLevelDirective {
  pub module_id: String,
  pub directive: String,
  pub source: ArcStr,
  pub span: Span,
}

impl BuildEvent for ModuleLevelDirective {
  fn kind(&self) -> EventKind {
    EventKind::ModuleLevelDirective
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "The semantics of the module level directive \"{}\" in \"{}\" may not be preserved when bundling.",
      self.directive,
      opts.stabilize_path(&self.module_id),
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.module_id);
    let file_id = diagnostic.add_file(filename, self.source.clone());
    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      String::from("module level directive may not be preserved"),
    );
    diagnostic.add_help(String::from(
      "For more information, see https://rolldown.rs/in-depth/directives#other-directives",
    ));
  }
}
