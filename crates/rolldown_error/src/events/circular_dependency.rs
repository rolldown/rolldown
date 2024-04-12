use super::BuildEvent;
use crate::event_kind::EventKind;

#[derive(Debug)]
pub struct CircularDependency {
  pub paths: Vec<String>,
}

impl BuildEvent for CircularDependency {
  fn kind(&self) -> EventKind {
    EventKind::CircularDependency
  }
  fn code(&self) -> &'static str {
    "CIRCULAR_DEPENDENCY"
  }

  fn message(&self) -> String {
    format!("Circular dependency: {}.", self.paths.join(" -> "))
  }
}
