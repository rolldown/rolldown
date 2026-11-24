use napi_derive::napi;

use rolldown_common::ModuleIdx;
use rolldown_plugin::SharedLoadPluginContext;

use super::binding_plugin_context::BindingPluginContext;

#[napi]
pub struct BindingLoadPluginContext {
  ctx: BindingPluginContext,
  module_idx: ModuleIdx,
}

#[napi]
impl BindingLoadPluginContext {
  pub fn new(inner: SharedLoadPluginContext) -> Self {
    Self { ctx: inner.inner.clone().into(), module_idx: inner.module_idx() }
  }

  #[napi]
  pub fn inner(&self) -> BindingPluginContext {
    self.ctx.clone()
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) -> napi::Result<()> {
    self.ctx.ctx()?.add_module_watch_file(self.module_idx, &file);
    Ok(())
  }
}
