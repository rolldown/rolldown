use std::sync::Arc;

use derive_more::Debug;
use napi::bindgen_prelude::FnArgs;
use rolldown::IsExternal;
use rolldown_plugin_esm_external_require::EsmExternalRequirePlugin;

use crate::types::js_callback::{JsCallback, JsCallbackExt as _};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingEsmExternalRequirePluginConfig {
  #[debug(skip)]
  #[napi(
    ts_type = "(source: string, importer: string | undefined, isResolved: boolean) => boolean"
  )]
  pub external: Option<JsCallback<FnArgs<(String, Option<String>, bool)>, bool>>,
}

impl From<BindingEsmExternalRequirePluginConfig> for EsmExternalRequirePlugin {
  fn from(config: BindingEsmExternalRequirePluginConfig) -> Self {
    let external = config.external.map(|is_external| {
      IsExternal::from_closure(move |source, importer, is_resolved| {
        let source = source.to_string();
        let importer = importer.map(ToString::to_string);
        let is_external = Arc::clone(&is_external);
        Box::pin(async move {
          is_external
            .invoke_async((source.to_string(), importer, is_resolved).into())
            .await
            .map_err(anyhow::Error::from)
        })
      })
    });
    Self { external: external.unwrap_or_default() }
  }
}
