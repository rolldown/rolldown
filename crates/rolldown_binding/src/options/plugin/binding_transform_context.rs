use std::sync::Arc;

use napi_derive::napi;

use rolldown_plugin::SharedTransformPluginContext;

use super::binding_plugin_context::BindingPluginContext;
use crate::types::binding_magic_string::BindingMagicString;
use crate::types::external_memory_status::ExternalMemoryStatus;

#[napi]
pub struct BindingTransformPluginContext {
  inner: Option<SharedTransformPluginContext>,
}

#[napi]
impl BindingTransformPluginContext {
  pub fn new(inner: SharedTransformPluginContext) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&SharedTransformPluginContext> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed: this transform context's native data was eagerly released after its hook invocation settled. Use the context only while the hook runs.",
      )
    })
  }

  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self) -> ExternalMemoryStatus {
    match self.inner.take() {
      None => ExternalMemoryStatus {
        freed: false,
        reason: Some("Memory has already been freed".to_string()),
      },
      Some(arc) => {
        let strong_count = Arc::strong_count(&arc);
        if strong_count > 1 {
          // Drop our reference, but others exist
          // Arc drops here automatically
          ExternalMemoryStatus {
            freed: false,
            reason: Some(format!(
              "Data has been dropped, but there are {} other strong reference(s) referring to this data on the native side, so the memory may not be released.",
              strong_count - 1
            )),
          }
        } else {
          // Last reference - memory will be freed
          // Arc drops here automatically
          ExternalMemoryStatus { freed: true, reason: None }
        }
      }
    }
  }

  #[napi]
  // TODO: should use `&str` instead. (claude code) Attempt failed due to performs JSON serialization to generate new String
  pub fn get_combined_sourcemap(&self) -> napi::Result<String> {
    Ok(self.try_get_inner()?.get_combined_sourcemap().to_json_string())
  }

  #[napi]
  pub fn inner(&self) -> napi::Result<BindingPluginContext> {
    Ok(self.try_get_inner()?.inner.clone().into())
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) -> napi::Result<()> {
    self.try_get_inner()?.add_watch_file(&file);
    Ok(())
  }

  #[napi]
  pub fn send_magic_string(
    &self,
    magic_string: &mut BindingMagicString,
  ) -> napi::Result<Option<String>> {
    // This moves the contents out, leaving `magic_string` unusable from JS onwards —
    // including for a repeated send, which errors here instead of queueing the empty
    // leftover into the sourcemap channel.
    let internal_magic_string = magic_string.take_inner()?;

    self.try_get_inner()?.send_magic_string(internal_magic_string).map_err(|_| {
      napi::Error::from_reason(
        "TransformPluginContext: failed to send MagicString to sourcemap worker - sourcemap \
         generation thread terminated unexpectedly during transform",
      )
    })
  }
}
