use sugar_path::SugarPath;

use super::BuildEvent;
use crate::{event_kind::EventKind, PathExt};

#[derive(Debug)]
pub struct CircularDependency {
  pub paths: Vec<String>,
}

impl CircularDependency {
  fn relative_paths(&self) -> Vec<String> {
    self.paths.iter().map(|p| p.as_path().relative_display().into_owned()).collect::<Vec<_>>()
  }
}

impl BuildEvent for CircularDependency {
  fn kind(&self) -> EventKind {
    EventKind::CircularDependency
  }
  fn code(&self) -> &'static str {
    "CIRCULAR_DEPENDENCY"
  }

  fn message(&self) -> String {
    format!("Circular dependency: {}.", self.relative_paths().join(" -> "))
  }
}
