use std::sync::{Arc, Weak};

use rolldown_common::{ModuleTable, ResolvedRequestInfo, SharedFileEmitter};
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
  pub(crate) file_emitter: SharedFileEmitter,
  #[allow(clippy::redundant_allocation)]
  pub(crate) module_table: Option<Arc<&'static mut ModuleTable>>,
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

  pub fn emit_file(&self, file: rolldown_common::EmittedAsset) -> String {
    self.file_emitter.emit_file(file)
  }

  pub fn get_file_name(&self, reference_id: &str) -> String {
    self.file_emitter.get_file_name(reference_id)
  }

  pub fn get_module_info(&self, module_id: &str) -> Option<rolldown_common::ModuleInfo> {
    self.module_table.as_ref().and_then(|module_table| {
      for normal_module in &module_table.modules {
        if let Some(ecma_module) = normal_module.as_ecma() {
          if ecma_module.resource_id.as_str() == module_id {
            return Some(ecma_module.to_module_info());
          }
        }
      }
      // TODO external module
      None
    })
  }

  pub fn get_module_ids(&self) -> Option<Vec<String>> {
    if let Some(module_table) = self.module_table.as_ref() {
      let mut ids = Vec::with_capacity(module_table.modules.len());
      for normal_module in &module_table.modules {
        ids.push(normal_module.resource_id().to_string());
      }
      // TODO external module
      Some(ids)
    } else {
      None
    }
  }
}
