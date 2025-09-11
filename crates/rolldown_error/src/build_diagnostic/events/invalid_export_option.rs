use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
use arcstr::ArcStr;

use super::BuildEvent;

#[derive(Debug)]
pub struct InvalidExportOption {
  pub export_mode: ArcStr,
  pub entry_module: ArcStr,
  pub export_keys: Vec<ArcStr>,
}

impl BuildEvent for InvalidExportOption {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    EventKind::InvalidExportOptionError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      r#""{}" was specified for "output.exports", but entry module "{}" has the following exports: {}."#,
      self.export_mode,
      &self.entry_module,
      &self.export_keys.iter().map(|k| format!(r#""{k}""#)).collect::<Vec<_>>().join(", ")
    )
  }
}
