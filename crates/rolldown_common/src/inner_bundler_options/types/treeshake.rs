use crate::types::js_regex::HybridRegex;

#[derive(Debug)]
pub enum TreeshakeOptions {
  False,
  Option(InnerOptions),
}

impl Default for TreeshakeOptions {
  /// Used for snapshot testing
  fn default() -> Self {
    TreeshakeOptions::Option(InnerOptions { module_side_effects: ModuleSideEffects::Boolean(true) })
  }
}

#[derive(Debug)]
pub enum ModuleSideEffects {
  Regex(HybridRegex),
  Boolean(bool),
}

impl ModuleSideEffects {
  pub fn resolve(&self, path: &str) -> bool {
    match self {
      ModuleSideEffects::Regex(reg) => reg.matches(path),
      ModuleSideEffects::Boolean(b) => *b,
    }
  }
}

impl TreeshakeOptions {
  pub fn enabled(&self) -> bool {
    matches!(self, TreeshakeOptions::Option(_))
  }
}

#[derive(Debug)]
pub struct InnerOptions {
  pub module_side_effects: ModuleSideEffects,
}
