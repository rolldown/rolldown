use std::sync::Arc;

use napi_derive::napi;

use rolldown_plugin::SharedPluginContext;

use crate::{types::binding_module_info::BindingModuleInfo, utils::napi_error};

use super::types::{
  binding_emitted_asset::BindingEmittedAsset,
  binding_plugin_context_resolve_options::BindingPluginContextResolveOptions,
};

#[napi]
pub struct BindingPluginContext {
  #[allow(dead_code)]
  inner: SharedPluginContext,
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
        &extra_options.unwrap_or_default().try_into().map_err(napi::Error::from_reason)?,
      )
      .await
      .map_err(|program_err| napi_error::resolve_error(&specifier, program_err))?
      .ok();
    Ok(ret.map(|info| BindingPluginContextResolvedId {
      id: info.path.path.to_string(),
      external: info.is_external,
    }))
  }

  #[napi]
  pub fn emit_file(&self, file: BindingEmittedAsset) -> String {
    self.inner.emit_file(file.into())
  }

  #[napi]
  pub fn get_file_name(&self, reference_id: String) -> String {
    self.inner.get_file_name(reference_id.as_str())
  }

  #[napi]
  pub fn get_module_info(&self, module_id: String) -> Option<BindingModuleInfo> {
    self.inner.get_module_info(&module_id).map(|info| BindingModuleInfo::new(Arc::new(info)))
  }

  #[napi]
  pub fn get_module_ids(&self) -> Option<Vec<String>> {
    self.inner.get_module_ids()
  }
}

impl From<SharedPluginContext> for BindingPluginContext {
  fn from(inner: SharedPluginContext) -> Self {
    Self { inner }
  }
}
#[napi(object)]
pub struct BindingPluginContextResolvedId {
  pub id: String,
  pub external: bool,
}
