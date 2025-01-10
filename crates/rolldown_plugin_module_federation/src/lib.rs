use std::borrow::Cow;

mod option;
pub use option::{ModuleFederationPluginOption, Remote, Shared};
use rolldown_plugin::Plugin;

#[derive(Debug)]
pub struct ModuleFederationPlugin {
  #[allow(dead_code)]
  options: ModuleFederationPluginOption,
}

impl ModuleFederationPlugin {
  pub fn new(options: ModuleFederationPluginOption) -> Self {
    Self { options }
  }
}

impl Plugin for ModuleFederationPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:module-federation")
  }
}
