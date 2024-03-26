use napi_derive::napi;
use std::path::PathBuf;

use rolldown_plugin::SharedPluginContext;

use super::types::binding_plugin_context_resolve_options::BindingPluginContextResolveOptions;

#[napi]
pub struct BindingPluginContext {
  #[allow(dead_code)]
  inner: SharedPluginContext,
}

#[napi]
impl BindingPluginContext {
  #[napi]
  pub fn resolve(
    &self,
    specifier: String,
    importer: Option<String>,
    extra_options: BindingPluginContextResolveOptions,
  ) -> napi::Result<()> {
    let importer = importer.map(PathBuf::from);
    self.inner.resolve(
      &specifier,
      importer.as_deref(),
      &extra_options.try_into().map_err(napi::Error::from_reason)?,
    );

    Ok(())
  }
}

impl From<SharedPluginContext> for BindingPluginContext {
  fn from(inner: SharedPluginContext) -> Self {
    Self { inner }
  }
}
