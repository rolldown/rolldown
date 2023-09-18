use std::path::PathBuf;

use derivative::Derivative;

#[derive(Debug)]
pub struct InputItem {
  pub name: Option<String>,
  pub import: String,
}

impl From<String> for InputItem {
  fn from(value: String) -> Self {
    Self {
      name: None,
      import: value,
    }
  }
}

#[derive(Derivative, Default)]
#[derivative(Debug)]
pub struct InputOptions {
  pub input: Option<Vec<InputItem>>,
  pub cwd: Option<PathBuf>,
}
