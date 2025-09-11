use arcstr::ArcStr;
use oxc::diagnostics::LabeledSpan;

use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
};

use super::BuildEvent;

#[derive(Debug)]
pub struct ParseError {
  pub(crate) source: ArcStr,
  pub(crate) filename: String,
  pub(crate) error_help: String,
  pub(crate) error_message: String,
  pub(crate) error_labels: Vec<LabeledSpan>,
}

impl BuildEvent for ParseError {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::ParseError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Parse failed, got: {:?}", self.error_message)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {
    diagnostic.title.clone_from(&self.error_message);

    let file_id = diagnostic.add_file(self.filename.clone(), self.source.clone());

    self.error_labels.iter().for_each(|label| {
      let offset = u32::try_from(label.offset()).unwrap();
      diagnostic.add_label(
        &file_id,
        offset..offset + u32::try_from(label.len()).unwrap(),
        label.label().unwrap_or(&String::default()).to_owned(),
      );
    });

    if !self.error_help.is_empty() {
      diagnostic.add_help(self.error_help.clone());
    }
  }
}
