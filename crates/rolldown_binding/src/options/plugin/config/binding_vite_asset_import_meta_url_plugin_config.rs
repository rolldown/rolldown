use std::{path::PathBuf, sync::Arc};

use napi::bindgen_prelude::FnArgs;
use rolldown_plugin_vite_asset_import_meta_url::ViteAssetImportMetaUrlPlugin;
use sugar_path::SugarPath as _;

use crate::{
  options::plugin::types::binding_asset_inline_limit::BindingAssetInlineLimit,
  types::js_callback::{
    JsCallback, JsCallbackExt as _, MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _,
  },
};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteAssetImportMetaUrlPluginConfig {
  pub root: String,
  pub is_lib: bool,
  pub public_dir: String,
  pub client_entry: String,
  #[napi(ts_type = "(id: string) => string | undefined")]
  pub try_fs_resolve: JsCallback<String, Option<String>>,
  #[napi(ts_type = "(id: string, importer: string) => Promise<string | undefined>")]
  pub asset_resolver: MaybeAsyncJsCallback<FnArgs<(String, String)>, Option<String>>,
  #[napi(ts_type = "number | ((file: string, content: Buffer) => boolean | undefined)")]
  pub asset_inline_limit: BindingAssetInlineLimit,
}

impl From<BindingViteAssetImportMetaUrlPluginConfig> for ViteAssetImportMetaUrlPlugin {
  fn from(value: BindingViteAssetImportMetaUrlPluginConfig) -> Self {
    Self {
      root: PathBuf::from(value.root).normalize(),
      is_lib: value.is_lib,
      public_dir: value.public_dir,
      client_entry: value.client_entry,
      asset_inline_limit: value.asset_inline_limit.into(),
      try_fs_resolve: Arc::new(move |id| {
        let id = id.to_string();
        let try_fs_resolve = Arc::clone(&value.try_fs_resolve);
        Box::pin(async move { try_fs_resolve.invoke_async(id).await.map_err(anyhow::Error::from) })
      }),
      asset_resolver: Arc::new(move |id: &str, importer: &str| {
        let id = id.to_string();
        let importer = importer.to_string();
        let asset_resolver = Arc::clone(&value.asset_resolver);
        Box::pin(async move {
          asset_resolver.await_call((id, importer).into()).await.map_err(anyhow::Error::from)
        })
      }),
    }
  }
}
