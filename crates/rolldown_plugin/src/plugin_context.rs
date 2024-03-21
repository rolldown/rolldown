use std::sync::Weak;

use crate::PluginDriver;

pub type SharedPluginContext = std::sync::Arc<PluginContext>;

#[derive(Debug, Default)]
pub struct PluginContext {
  pub(crate) _plugin_driver: Weak<PluginDriver>,
}
