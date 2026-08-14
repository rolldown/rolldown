use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct RuntimeModuleSymbolNotFound {
  pub symbol_names: Vec<String>,
  pub modified_by_plugins: Vec<String>,
}

impl BuildEvent for RuntimeModuleSymbolNotFound {
  fn kind(&self) -> EventKind {
    EventKind::RuntimeModuleSymbolNotFoundError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    let symbols =
      self.symbol_names.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
    if self.modified_by_plugins.is_empty() {
      format!(
        "Failed to resolve runtime symbol(s) {symbols}. This is a Rolldown internal error - please file an issue at https://github.com/rolldown/rolldown/issues.",
      )
    } else {
      format!(
        "Failed to resolve runtime symbol(s) {symbols} after the runtime module was modified by plugin(s): {}. Please review these plugins to ensure they do not accidentally remove or rename runtime utilities.",
        self.modified_by_plugins.join(", ")
      )
    }
  }
}
