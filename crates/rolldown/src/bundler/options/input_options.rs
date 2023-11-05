use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

use derivative::Derivative;
use rolldown_fs::FileSystem;

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
  pub input: Option<Vec<InputItem>>,
  pub cwd: Option<PathBuf>,
  #[derivative(Debug = "ignore")]
  pub fs: Arc<dyn FileSystem>,
}
