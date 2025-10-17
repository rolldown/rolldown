use std::sync::Arc;

use napi::bindgen_prelude::{Buffer, Either, FnArgs};
use rolldown_plugin_utils::UsizeOrFunction;
use rolldown_plugin_vite_css::{UrlResolver, ViteCSSPlugin};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::types::{
  binding_sourcemap::BindingSourcemap,
  js_callback::{
    JsCallback, JsCallbackExt as _, MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _,
  },
};

#[expect(dead_code)]
#[napi_derive::napi(object)]
pub struct BindingCompileCSSResult {
  pub code: String,
  pub map: Option<BindingSourcemap>,
  pub deps: Option<FxHashSet<String>>,
  pub modules: Option<FxHashMap<String, String>>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(derive_more::Debug)]
pub struct BindingViteCSSPluginConfig {
  pub is_lib: bool,
  pub public_dir: String,
  #[debug(skip)]
  #[napi(ts_type = "(url: string, importer?: string) => MaybePromise<string | undefined>")]
  pub resolve_url: MaybeAsyncJsCallback<FnArgs<(String, Option<String>)>, Option<String>>,
  #[debug(skip)]
  #[napi(ts_type = "number | ((file: string, content: Buffer) => boolean | undefined)")]
  pub asset_inline_limit: Option<Either<u32, JsCallback<FnArgs<(String, Buffer)>, Option<bool>>>>,
}

impl From<BindingViteCSSPluginConfig> for ViteCSSPlugin {
  fn from(value: BindingViteCSSPluginConfig) -> Self {
    let asset_inline_limit =
      value.asset_inline_limit.map(|asset_inline_limit| match asset_inline_limit {
        Either::A(value) => UsizeOrFunction::Number(value as usize),
        Either::B(func) => {
          UsizeOrFunction::Function(Arc::new(move |file: &str, content: &[u8]| {
            let file = file.to_string();
            let content = Buffer::from(content);
            let asset_inline_limit_fn = Arc::clone(&func);
            Box::pin(async move {
              asset_inline_limit_fn
                .invoke_async((file, content).into())
                .await
                .map_err(anyhow::Error::from)
            })
          }))
        }
      });

    Self {
      is_lib: value.is_lib,
      public_dir: value.public_dir,
      compile_css: Arc::new(move |url: &str, importer: &str, _url_resolver: Arc<UrlResolver>| {
        let _url = url.to_string();
        let _importer = importer.to_string();
        Box::pin(async move { todo!() })
      }),
      resolve_url: Arc::new(move |url: &str, importer: Option<&str>| {
        let url = url.to_string();
        let importer = importer.map(String::from);
        let resolve_url = Arc::clone(&value.resolve_url);
        Box::pin(async move {
          resolve_url.await_call((url, importer).into()).await.map_err(anyhow::Error::from)
        })
      }),
      asset_inline_limit: asset_inline_limit.unwrap_or_default(),
    }
  }
}
