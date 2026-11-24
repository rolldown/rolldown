use std::sync::{Arc, Weak};

use napi_derive::napi;

use rolldown_common::ModuleIdx;
use rolldown_plugin::{
  COMBINED_SOURCEMAP_AFTER_RELEASE_ERROR, SEND_AFTER_HOOK_ERROR, SharedTransformPluginContext,
  TransformPluginContext,
};

use super::binding_plugin_context::BindingPluginContext;
use crate::types::binding_magic_string::BindingMagicString;

#[napi]
pub struct BindingTransformPluginContext {
  // The plugin driver keeps it only while the module's sourcemap chain is unchanged. `ctx` stays
  // usable until the build finishes.
  inner: Weak<TransformPluginContext>,
  ctx: BindingPluginContext,
  module_idx: ModuleIdx,
  is_virtual_module: bool,
}

#[napi]
impl BindingTransformPluginContext {
  pub fn new(inner: SharedTransformPluginContext) -> Self {
    Self {
      ctx: inner.inner.clone().into(),
      module_idx: inner.module_idx(),
      is_virtual_module: inner.id().starts_with('\0'),
      inner: Arc::downgrade(&inner),
    }
  }

  #[napi]
  // TODO: should use `&str` instead. (claude code) Attempt failed due to performs JSON serialization to generate new String
  pub fn get_combined_sourcemap(&self) -> napi::Result<String> {
    let inner = self
      .inner
      .upgrade()
      .ok_or_else(|| napi::Error::from_reason(COMBINED_SOURCEMAP_AFTER_RELEASE_ERROR))?;
    Ok(inner.get_combined_sourcemap()?.to_json_string())
  }

  #[napi]
  pub fn inner(&self) -> BindingPluginContext {
    self.ctx.clone()
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) -> napi::Result<()> {
    // Virtual modules can't be refetched from disk during HMR.
    if self.is_virtual_module {
      return Ok(());
    }
    self.ctx.ctx()?.add_module_watch_file(self.module_idx, &file);
    Ok(())
  }

  #[napi]
  pub fn send_magic_string(
    &self,
    magic_string: &mut BindingMagicString<'static>,
  ) -> napi::Result<Option<String>> {
    // This moves the contents out, leaving `magic_string` unusable from JS onwards —
    // including for a repeated send, which errors here instead of registering the
    // empty leftover as the hook's sourcemap.
    let internal_magic_string = magic_string.take_inner()?;

    let inner =
      self.inner.upgrade().ok_or_else(|| napi::Error::from_reason(SEND_AFTER_HOOK_ERROR))?;
    Ok(inner.send_magic_string(internal_magic_string)?)
  }
}
