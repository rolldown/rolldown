use crate::events::BuildEvent;
use crate::{DiagnosticOptions, EventKind};
use arcstr::ArcStr;

#[derive(Debug)]
pub struct MixedExport {
  pub module_name: ArcStr,
  pub entry_module: ArcStr,
  pub export_keys: Vec<ArcStr>,
}

impl BuildEvent for MixedExport {
  fn kind(&self) -> EventKind {
    EventKind::MixedExport
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
