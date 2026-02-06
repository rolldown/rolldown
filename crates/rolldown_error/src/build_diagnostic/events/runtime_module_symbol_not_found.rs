use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct RuntimeModuleSymbolNotFound {
  pub symbol_name: String,
  pub modified_by_plugins: Vec<String>,
}

impl BuildEvent for RuntimeModuleSymbolNotFound {
  fn kind(&self) -> EventKind {
    EventKind::RuntimeModuleSymbolNotFoundError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    if self.modified_by_plugins.is_empty() {
      format!(
        "Failed to resolve runtime symbol \"{}\". This is a Rolldown internal error - please file an issue at https://github.com/rolldown/rolldown/issues.",
        self.symbol_name
      )
    } else {
      format!(
        "Failed to resolve runtime symbol \"{}\" after the runtime module was modified by plugin(s): {}. Please review these plugins to ensure they do not accidentally remove or rename runtime utilities.",
        self.symbol_name,
        self.modified_by_plugins.join(", ")
      )
    }
  }
}
