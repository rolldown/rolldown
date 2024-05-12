use napi_derive::napi;

use rolldown_plugin::SharedPluginContext;

use crate::utils::napi_error;

use super::types::binding_plugin_context_resolve_options::BindingPluginContextResolveOptions;

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
