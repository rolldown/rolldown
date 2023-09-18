use std::path::PathBuf;

use derivative::Derivative;

use super::input_options::InputItem;
use crate::InputOptions;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct NormalizedInputOptions {
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
}

impl NormalizedInputOptions {
  pub fn from_input_options(opts: InputOptions) -> Self {
    Self {
      input: opts.input.unwrap_or_default(),
      cwd: opts.cwd.unwrap_or_else(|| std::env::current_dir().unwrap()),
    }
  }
}
