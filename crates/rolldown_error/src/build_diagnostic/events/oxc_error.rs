use arcstr::ArcStr;
use oxc::diagnostics::LabeledSpan;

use crate::{
  build_diagnostic::diagnostic::Diagnostic,
  types::{diagnostic_options::DiagnosticOptions, event_kind::EventKind},
};

use super::BuildEvent;

#[derive(derive_more::Debug)]
pub struct OxcError {
  pub(crate) source: ArcStr,
  pub(crate) id: String,
  pub(crate) error_help: String,
  pub(crate) error_message: String,
  pub(crate) error_labels: Vec<LabeledSpan>,
  #[debug(skip)]
  pub(crate) event_kind: EventKind,
}

impl BuildEvent for OxcError {
  fn kind(&self) -> EventKind {
    self.event_kind
  }

  fn id(&self) -> Option<String> {
    Some(self.id.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    self.error_message.clone()
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    diagnostic.title.clone_from(&self.error_message);

    let file_id = diagnostic.add_file(opts.stabilize_path(&self.id), self.source.clone());

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
