use std::{path::Path, sync::Weak};

use crate::{types::plugin_context_resolve_options::PluginContextResolveOptions, PluginDriver};

pub type SharedPluginContext = std::sync::Arc<PluginContext>;

#[derive(Debug, Default)]
pub struct PluginContext {
  pub(crate) _plugin_driver: Weak<PluginDriver>,
}

impl PluginContext {
  pub fn resolve(
    &self,
    _specifier: &str,
    _importer: Option<&Path>,
    _extra_options: &PluginContextResolveOptions,
  ) {
    unimplemented!()
  }
}
