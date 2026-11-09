use napi_derive::napi;

use rolldown_plugin::SharedLoadPluginContext;

use super::binding_plugin_context::BindingPluginContext;
use crate::types::js_callback_release::JsCallbackUser;

#[napi]
pub struct BindingLoadPluginContext {
  inner: SharedLoadPluginContext,
  /// Goes to the context that `inner()` returns. See [`JsCallbackUser`].
  callback_user: Option<JsCallbackUser>,
}

#[napi]
impl BindingLoadPluginContext {
  pub fn new(inner: SharedLoadPluginContext, callback_user: Option<JsCallbackUser>) -> Self {
    Self { inner, callback_user }
  }

  #[napi]
  pub fn inner(&self) -> BindingPluginContext {
    BindingPluginContext::new(self.inner.inner.clone(), self.callback_user.clone())
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) {
    self.inner.add_watch_file(&file);
  }
}
