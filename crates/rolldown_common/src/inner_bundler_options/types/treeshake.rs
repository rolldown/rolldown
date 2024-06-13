#[derive(Debug)]
pub enum TreeshakeOptions {
  False,
  Option(InnerOptions),
}

impl Default for TreeshakeOptions {
  /// Used for snapshot testing
  fn default() -> Self {
    TreeshakeOptions::Option(InnerOptions { module_side_effects: true })
  }
}

impl TreeshakeOptions {
  pub fn enabled(&self) -> bool {
    matches!(self, TreeshakeOptions::Option(_))
  }
}

#[derive(Debug)]
pub struct InnerOptions {
  pub module_side_effects: bool,
}
