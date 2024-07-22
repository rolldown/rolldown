use crate::OutputExports;

/// This is the result after determining what the exports mode is for a module, and the keypoint is to handle with the `auto` mode.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ExportMode {
  Default,
  Named,
  None,
}

impl ExportMode {
  pub fn is_named(&self) -> bool {
    matches!(self, Self::Named)
  }

  pub fn is_default(&self) -> bool {
    matches!(self, Self::Default)
  }

  pub fn is_none(&self) -> bool {
    matches!(self, Self::None)
  }
}

impl From<OutputExports> for ExportMode {
  fn from(exports: OutputExports) -> Self {
    match exports {
      OutputExports::Default => Self::Default,
      OutputExports::Named => Self::Named,
      OutputExports::None => Self::None,
      OutputExports::Auto => {
        unreachable!("`output.exports` must be resolved before this conversion")
      }
    }
  }
}
