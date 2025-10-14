use napi_derive::napi;

use rolldown_plugin::SharedTransformPluginContext;

use super::binding_plugin_context::BindingPluginContext;
use crate::types::binding_magic_string::BindingMagicString;

#[napi]
pub struct BindingTransformPluginContext {
  inner: SharedTransformPluginContext,
}

#[napi]
impl BindingTransformPluginContext {
  pub fn new(inner: SharedTransformPluginContext) -> Self {
    Self { inner }
  }

  #[napi]
  // TODO: claude code - Cannot change to &str: performs JSON serialization to generate new String
  pub fn get_combined_sourcemap(&self) -> String {
    self.inner.get_combined_sourcemap().to_json_string()
  }

  #[napi]
  pub fn inner(&self) -> BindingPluginContext {
    self.inner.inner.clone().into()
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) {
    self.inner.add_watch_file(&file);
  }

  #[napi]
  pub fn send_magic_string(
    &self,
    magic_string: &mut BindingMagicString<'static>,
  ) -> Option<String> {
    let internal_magic_string = std::mem::take(&mut magic_string.inner);

    // If the the message is not send to main thread correctly, we should panic immediately.
    self.inner.send_magic_string(internal_magic_string).expect(
      "TransformPluginContext: failed to send MagicString to sourcemap worker - sourcemap generation thread terminated unexpectedly during transform"
    )
  }
}
