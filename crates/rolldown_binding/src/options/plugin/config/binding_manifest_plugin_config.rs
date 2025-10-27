use std::sync::Arc;

use rolldown_plugin_manifest::{IsLegacyFn, ManifestPlugin};
use rustc_hash::FxHashMap;

use crate::types::js_callback::{JsCallback, JsCallbackExt as _};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingManifestPluginConfig {
  pub root: String,
  pub out_path: String,
  #[napi(ts_type = "() => boolean")]
  pub is_legacy: Option<JsCallback<(), bool>>,
  #[napi(ts_type = "() => Record<string, string>")]
  pub css_entries: JsCallback<(), FxHashMap<String, String>>,
}

impl From<BindingManifestPluginConfig> for ManifestPlugin {
  fn from(value: BindingManifestPluginConfig) -> Self {
    Self {
      root: value.root,
      out_path: value.out_path,
      is_legacy: value.is_legacy.map(|cb| -> Arc<IsLegacyFn> {
        Arc::new(move || {
          let is_legacy_fn = Arc::clone(&cb);
          Box::pin(async move { is_legacy_fn.invoke_async(()).await.map_err(anyhow::Error::from) })
        })
      }),
      css_entries: Arc::new(move || {
        let css_entries_fn = Arc::clone(&value.css_entries);
        Box::pin(async move { css_entries_fn.invoke_async(()).await.map_err(anyhow::Error::from) })
      }),
    }
  }
}
