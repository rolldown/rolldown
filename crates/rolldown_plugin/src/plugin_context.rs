use std::sync::Weak;

use crate::PluginDriver;

#[derive(Debug, Default)]
pub struct PluginContext {
  pub(crate) _plugin_driver: Weak<PluginDriver>,
}
