use std::sync::{Arc, Weak};

use rolldown_common::{ResolvedRequestInfo, SharedFileEmitter};
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
}
