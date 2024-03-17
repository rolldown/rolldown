//! [crate::InputOptions] meant to provide dx-friendly options for the `rolldown` users, but it's not suitable for
//! the `rolldown` internal use.

use std::{path::PathBuf, sync::Arc};

use derivative::Derivative;

use crate::External;

use super::types::input_item::InputItem;

pub type SharedNormalizedInputOptions = Arc<NormalizedInputOptions>;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct NormalizedInputOptions {
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub external: External,
  pub treeshake: bool,
}
