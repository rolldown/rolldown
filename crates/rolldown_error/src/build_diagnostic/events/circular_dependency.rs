use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct CircularDependency {
  pub paths: Vec<String>,
}

impl CircularDependency {
  fn stable_paths(&self, opts: &DiagnosticOptions) -> Vec<String> {
    self.paths.iter().map(|p| opts.stabilize_path(p)).collect::<Vec<_>>()
  }
}

impl BuildEvent for CircularDependency {
  fn kind(&self) -> EventKind {
    EventKind::CircularDependency
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!("Circular dependency: {}.", self.stable_paths(opts).join(" -> "))
  }
}
