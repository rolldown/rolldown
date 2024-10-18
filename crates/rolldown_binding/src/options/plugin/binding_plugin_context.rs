use std::sync::Arc;

use napi_derive::napi;

use rolldown_plugin::PluginContext;

use crate::{types::binding_module_info::BindingModuleInfo, utils::napi_error};

use super::types::{
  binding_emitted_asset::BindingEmittedAsset,
  binding_plugin_context_resolve_options::BindingPluginContextResolveOptions,
};

#[napi]
pub struct BindingPluginContext {
  #[allow(dead_code)]
  inner: PluginContext,
}

#[napi]
impl BindingPluginContext {
  #[napi]
  pub async fn resolve(
    &self,
    specifier: String,
    importer: Option<String>,
    extra_options: Option<BindingPluginContextResolveOptions>,
  ) -> napi::Result<Option<BindingPluginContextResolvedId>> {
    let ret = self
      .inner
      .resolve(
        &specifier,
        importer.as_deref(),
        extra_options.map(TryInto::try_into).transpose().map_err(napi::Error::from_reason)?,
      )
      .await
      .map_err(|program_err| napi_error::resolve_error(&specifier, program_err))?
      .ok();
    Ok(ret.map(|info| BindingPluginContextResolvedId {
      id: info.id.to_string(),
      external: info.is_external,
    }))
  }

  #[napi]
  pub fn emit_file(&self, file: BindingEmittedAsset) -> String {
    self.inner.emit_file(file.into()).to_string()
  }

  #[napi]
  pub fn get_file_name(&self, reference_id: String) -> String {
    self.inner.get_file_name(reference_id.as_str()).to_string()
  }

  #[napi]
  pub fn get_module_info(&self, module_id: String) -> Option<BindingModuleInfo> {
    self.inner.get_module_info(&module_id).map(|info| BindingModuleInfo::new(Arc::new(info)))
  }

  #[napi]
  pub fn get_module_ids(&self) -> Option<Vec<String>> {
    self.inner.get_module_ids()
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) {
    self.inner.add_watch_file(&file);
  }
}

impl From<PluginContext> for BindingPluginContext {
  fn from(inner: PluginContext) -> Self {
    Self { inner }
  }
}
#[napi(object)]
pub struct BindingPluginContextResolvedId {
  pub id: String,
  pub external: bool,
}
