use super::BuildErrorLike;

#[derive(Debug)]
pub struct CircularDependency {
  pub paths: Vec<String>,
}

impl BuildErrorLike for CircularDependency {
  fn code(&self) -> &'static str {
    "CIRCULAR_DEPENDENCY"
  }

  fn message(&self) -> String {
    format!("Circular dependency: {}.", self.paths.join(" -> "))
  }
}
