use std::fmt::Debug;
use std::path::PathBuf;

use derivative::Derivative;

#[derive(Debug)]
pub struct InputItem {
  pub name: Option<String>,
  pub import: String,
}

impl From<String> for InputItem {
  fn from(value: String) -> Self {
    Self { name: None, import: value }
  }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct InputOptions {
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
}

impl Default for InputOptions {
  fn default() -> Self {
    Self { input: vec![], cwd: std::env::current_dir().unwrap() }
  }
}
