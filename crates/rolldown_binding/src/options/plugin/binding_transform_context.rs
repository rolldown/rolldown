use napi_derive::napi;

use rolldown_plugin::SharedTransformPluginContext;

use super::binding_plugin_context::BindingPluginContext;
use crate::types::{binding_magic_string::BindingMagicString, js_callback_release::JsCallbackUser};

#[napi]
pub struct BindingTransformPluginContext {
  inner: SharedTransformPluginContext,
  /// Goes to the context that `inner()` returns. See [`JsCallbackUser`].
  callback_user: Option<JsCallbackUser>,
}

#[napi]
impl BindingTransformPluginContext {
  pub fn new(inner: SharedTransformPluginContext, callback_user: Option<JsCallbackUser>) -> Self {
    Self { inner, callback_user }
  }

  #[napi]
  // TODO: should use `&str` instead. (claude code) Attempt failed due to performs JSON serialization to generate new String
  pub fn get_combined_sourcemap(&self) -> String {
    self.inner.get_combined_sourcemap().to_json_string()
  }

  #[napi]
  pub fn inner(&self) -> BindingPluginContext {
    BindingPluginContext::new(self.inner.inner.clone(), self.callback_user.clone())
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) {
    self.inner.add_watch_file(&file);
  }

  #[napi]
  pub fn send_magic_string(
    &self,
    magic_string: &mut BindingMagicString<'static>,
  ) -> napi::Result<Option<String>> {
    // This moves the contents out, leaving `magic_string` unusable from JS onwards —
    // including for a repeated send, which errors here instead of queueing the empty
    // leftover into the sourcemap channel.
    let internal_magic_string = magic_string.take_inner()?;

    self.inner.send_magic_string(internal_magic_string).map_err(|_| {
      napi::Error::from_reason(
        "TransformPluginContext: failed to send MagicString to sourcemap worker - sourcemap \
         generation thread terminated unexpectedly during transform",
      )
    })
  }
}
