use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug)]
pub struct FileNotFound {
  /// The reference id that no emitted file matches.
  pub reference_id: String,
  pub module_id: String,
  pub source: ArcStr,
  /// Span of the `import.meta.ROLLDOWN_FILE_URL_<referenceId>` access within the module source.
  pub span: Span,
}

impl BuildEvent for FileNotFound {
  fn kind(&self) -> EventKind {
    EventKind::FileNotFoundError
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Plugin error - Unable to get file name for unknown file \"{}\".", self.reference_id)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.module_id);
    let file_id = diagnostic.add_file(filename, self.source.clone());

    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      String::from("no emitted file has this reference id"),
    );

    diagnostic.add_help(String::from("Reference ids come from `this.emitFile()`."));
  }
}
