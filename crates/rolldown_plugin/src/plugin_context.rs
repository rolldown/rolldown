use std::sync::{Arc, Weak};

use rolldown_common::{ModuleInfo, ResolvedRequestInfo, SharedModuleTable};
use rolldown_resolver::{ResolveError, Resolver};

use crate::{
  types::plugin_context_resolve_options::PluginContextResolveOptions,
  utils::resolve_id_with_plugins::resolve_id_with_plugins, HookResolveIdExtraOptions, PluginDriver,
};

pub type SharedPluginContext = std::sync::Arc<PluginContext>;

#[derive(Debug)]
pub struct PluginContext {
  pub(crate) resolver: Arc<Resolver>,
  pub(crate) plugin_driver: Weak<PluginDriver>,
  pub(crate) module_table: SharedModuleTable,
}

impl PluginContext {
  pub async fn resolve(
    &self,
    specifier: &str,
    importer: Option<&str>,
    extra_options: &PluginContextResolveOptions,
  ) -> anyhow::Result<Result<ResolvedRequestInfo, ResolveError>> {
    let plugin_driver = self
      .plugin_driver
      .upgrade()
      .ok_or_else(|| anyhow::format_err!("Plugin driver is already dropped."))?;

    resolve_id_with_plugins(
      &self.resolver,
      &plugin_driver,
      specifier,
      importer,
      HookResolveIdExtraOptions { is_entry: false, kind: extra_options.import_kind },
    )
    .await
  }

  pub fn get_module_info(&self, module_id: &str) -> Option<ModuleInfo> {
    let module_table = self.module_table.read().expect("should get module table read lock");

    for module in &module_table.normal_modules {
      if module.resource_id.as_str() == module_id {
        return Some(module.to_module_info());
      }
    }
    // TODO handle external modules
    None
  }
}
