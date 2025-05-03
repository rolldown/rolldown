use napi_derive::napi;

use rolldown_plugin::{PluginContext, SharedNativePluginContext};

use super::types::{
  binding_emitted_asset::BindingEmittedAsset, binding_emitted_chunk::BindingEmittedChunk,
  binding_hook_side_effects::BindingHookSideEffects,
  binding_plugin_context_resolve_options::BindingPluginContextResolveOptions,
  binding_resolved_external::BindingResolvedExternal,
};

use crate::{types::binding_module_info::BindingModuleInfo, utils::napi_error};

#[napi]
pub struct BindingPluginContext {
  inner: SharedNativePluginContext,
}

#[napi]
impl BindingPluginContext {
  #[napi(ts_args_type = "specifier: string, sideEffects: BindingHookSideEffects | undefined")]
  pub async fn load(
    &self,
    specifier: String,
    side_effects: Option<BindingHookSideEffects>,
  ) -> napi::Result<()> {
    self
      .inner
      .load(&specifier, side_effects.map(Into::into))
      .await
      .map_err(|program_err| napi_error::load_error(&specifier, program_err))
  }

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
      external: info.external.into(),
    }))
  }

  #[napi]
  pub fn emit_file(
    &self,
    file: BindingEmittedAsset,
    asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  ) -> String {
    self.inner.emit_file(file.into(), asset_filename, fn_sanitized_file_name).to_string()
  }

  #[napi]
  pub fn emit_chunk(&self, file: BindingEmittedChunk) -> anyhow::Result<String> {
    futures::executor::block_on(async {
      self.inner.emit_chunk(file.into()).await.map(|id| id.to_string())
    })
  }

  #[napi]
  pub fn get_file_name(&self, reference_id: String) -> anyhow::Result<String> {
    self.inner.get_file_name(reference_id.as_str()).map(|id| id.to_string())
  }

  #[napi]
  pub fn get_module_info(&self, module_id: String) -> Option<BindingModuleInfo> {
    self.inner.get_module_info(&module_id).map(BindingModuleInfo::new)
  }

  #[napi]
  pub fn get_module_ids(&self) -> Vec<String> {
    self.inner.get_module_ids()
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) {
    self.inner.add_watch_file(&file);
  }
}

impl From<PluginContext> for BindingPluginContext {
  fn from(ctx: PluginContext) -> Self {
    match ctx {
      PluginContext::Napi(_) => unreachable!("Js plugins don't have PluginContext::Napi"),
      PluginContext::Native(inner) => Self { inner },
    }
  }
}
#[napi(object)]
pub struct BindingPluginContextResolvedId {
  pub id: String,
  #[napi(ts_type = "boolean | 'absolute' | 'relative'")]
  pub external: BindingResolvedExternal,
}
