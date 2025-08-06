use std::sync::Arc;

use rolldown_plugin_manifest::ManifestPlugin;
use rustc_hash::FxHashSet;

use crate::types::js_callback::{JsCallback, JsCallbackExt as _};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingManifestPluginConfig {
  pub root: String,
  pub out_path: String,
  #[napi(ts_type = "() => Set<string>")]
  pub css_entries: JsCallback<(), FxHashSet<String>>,
}

impl From<BindingManifestPluginConfig> for ManifestPlugin {
  fn from(value: BindingManifestPluginConfig) -> Self {
    Self {
      root: value.root,
      out_path: value.out_path,
      css_entries: Arc::new(move || {
        let css_entries_fn = Arc::clone(&value.css_entries);
        Box::pin(async move { css_entries_fn.invoke_async(()).await.map_err(anyhow::Error::from) })
      }),
    }
  }
}
