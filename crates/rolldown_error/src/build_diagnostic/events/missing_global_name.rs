use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
use arcstr::ArcStr;

#[derive(Debug)]
pub struct MissingGlobalName {
  pub module_id: String,
  pub module_name: ArcStr,
  pub guessed_name: ArcStr,
}

impl BuildEvent for MissingGlobalName {
  fn kind(&self) -> EventKind {
    EventKind::MissingGlobalName
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      r#"No name was provided for external module "{}" in "output.globals" – guessing "{}"."#,
      &self.module_name, &self.guessed_name
    )
  }
}
