use std::borrow::Cow;

use rolldown_plugin::Plugin;

#[derive(Debug)]
pub struct ModuleFederationPlugin {}

impl ModuleFederationPlugin {
  pub fn new() -> Self {
    Self {}
  }
}

impl Plugin for ModuleFederationPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:module-federation")
  }
}
