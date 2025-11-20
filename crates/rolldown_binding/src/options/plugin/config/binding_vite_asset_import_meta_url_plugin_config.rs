use std::sync::Arc;

use rolldown_plugin_vite_asset_import_meta_url::ViteAssetImportMetaUrlPlugin;

use crate::types::js_callback::{JsCallback, JsCallbackExt as _};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteAssetImportMetaUrlPluginConfig {
  pub client_entry: String,
  #[napi(ts_type = "(id: string, importer: string) => string | undefined")]
  pub try_fs_resolve: JsCallback<String, Option<String>>,
}

impl From<BindingViteAssetImportMetaUrlPluginConfig> for ViteAssetImportMetaUrlPlugin {
  fn from(value: BindingViteAssetImportMetaUrlPluginConfig) -> Self {
    Self {
      client_entry: value.client_entry,
      try_fs_resolve: Arc::new(move |id| {
        let id = id.to_string();
        let try_fs_resolve = Arc::clone(&value.try_fs_resolve);
        Box::pin(async move { try_fs_resolve.invoke_async(id).await.map_err(anyhow::Error::from) })
      }),
    }
  }
}
