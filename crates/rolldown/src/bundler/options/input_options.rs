use std::fmt::Debug;
use std::path::PathBuf;

use derivative::Derivative;

pub type ExternalFn = dyn Fn(String, Option<String>, bool) -> bool;

pub enum External {
  String(String),
  Fn(Box<ExternalFn>),
}

impl Debug for External {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      External::String(value) => write!(f, "External::String({:?})", value),
      External::Fn(_) => write!(f, "External::Fn(...)"),
    }
  }
}

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
  pub external: Vec<External>,
}

impl Default for InputOptions {
  fn default() -> Self {
    Self { input: vec![], cwd: std::env::current_dir().unwrap(), external: vec![] }
  }
}
