use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct ManualCodeSplittingCircularChunkDependency {
  pub module_id: String,
  pub group_name: String,
}

impl BuildEvent for ManualCodeSplittingCircularChunkDependency {
  fn kind(&self) -> EventKind {
    EventKind::ManualCodeSplittingSkipped
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let module_id = opts.stabilize_path(&self.module_id);
    format!(
      "Skipped manual code splitting for \"{module_id}\" into group \"{}\" because it would create a circular chunk dependency and may cause TDZ errors at runtime.\n\
       \n\
       To keep the split, consider:\n\
       \n\
       - Enabling `strictExecutionOrder: true` (wraps modules with lazy init)\n\
       - Setting `manualCodeSplitting.includeDependenciesRecursively: true`",
      self.group_name
    )
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }
}
