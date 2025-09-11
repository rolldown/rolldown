use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
use arcstr::ArcStr;

#[derive(Debug)]
pub struct MixedExport {
  pub module_id: String,
  pub module_name: ArcStr,
  pub entry_module: String,
  pub export_keys: Vec<ArcStr>,
}

impl BuildEvent for MixedExport {
  fn kind(&self) -> EventKind {
    EventKind::MixedExport
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      r#"Entry module "{}" is using named (including {}) and default exports together. Consumers of your bundle will have to use `{}.default` to access the default export, which may not be what you want. Use `output.exports: "named"` to disable this warning."#,
      &self.entry_module,
      &self.export_keys.iter().map(|k| format!(r#""{k}""#)).collect::<Vec<_>>().join(", "),
      &self.module_name
    )
  }
}
