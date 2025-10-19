use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use rolldown_plugin_vite_css::{CompileCSSResult, UrlResolver, ViteCSSPlugin};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  options::plugin::types::binding_asset_inline_limit::BindingAssetInlineLimit,
  types::{
    binding_sourcemap::BindingSourcemap,
    js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _},
  },
};

#[napi_derive::napi]
pub struct BindingUrlResolver {
  inner: Arc<UrlResolver>,
}

impl BindingUrlResolver {
  pub fn new(inner: Arc<UrlResolver>) -> Self {
    Self { inner }
  }
}

#[napi_derive::napi]
impl BindingUrlResolver {
  #[napi(
    ts_args_type = "url: string, importer?: string",
    ts_return_type = "Promise<[string, string | undefined]>"
  )]
  pub async fn call(
    &self,
    url: String,
    importer: Option<String>,
  ) -> napi::Result<(String, Option<String>)> {
    (self.inner)(url, importer).await.map_err(napi::Error::from)
  }
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingCompileCSSResult {
  pub code: String,
  pub map: Option<BindingSourcemap>,
  pub deps: Option<FxHashSet<String>>,
  pub modules: Option<FxHashMap<String, String>>,
}

impl TryFrom<BindingCompileCSSResult> for CompileCSSResult {
  type Error = anyhow::Error;

  fn try_from(value: BindingCompileCSSResult) -> Result<Self, Self::Error> {
    Ok(Self {
      code: value.code,
      map: value.map.map(TryInto::try_into).transpose()?,
      deps: value.deps,
      modules: value.modules,
    })
  }
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteCSSPluginConfig {
  pub is_lib: bool,
  pub public_dir: String,
  #[napi(
    js_name = "compileCSS",
    ts_type = "(url: string, importer: string, resolver: BindingUrlResolver) => Promise<{
  code: string
  map?: BindingSourcemap
  modules?: Record<string, string>
  deps?: Set<string>
}>"
  )]
  pub compile_css:
    MaybeAsyncJsCallback<FnArgs<(String, String, BindingUrlResolver)>, BindingCompileCSSResult>,
  #[napi(ts_type = "(url: string, importer?: string) => MaybePromise<string | undefined>")]
  pub resolve_url: MaybeAsyncJsCallback<FnArgs<(String, Option<String>)>, Option<String>>,
  #[napi(ts_type = "number | ((file: string, content: Buffer) => boolean | undefined)")]
  pub asset_inline_limit: BindingAssetInlineLimit,
}

impl From<BindingViteCSSPluginConfig> for ViteCSSPlugin {
  fn from(value: BindingViteCSSPluginConfig) -> Self {
    Self {
      is_lib: value.is_lib,
      public_dir: value.public_dir,
      compile_css: Arc::new(move |url: &str, importer: &str, url_resolver: Arc<UrlResolver>| {
        let url = url.to_string();
        let importer = importer.to_string();
        let compile_css = Arc::clone(&value.compile_css);
        Box::pin(async move {
          compile_css
            .await_call((url, importer, BindingUrlResolver::new(url_resolver)).into())
            .await
            .map_err(anyhow::Error::from)
            .and_then(TryInto::try_into)
        })
      }),
      resolve_url: Arc::new(move |url: &str, importer: Option<&str>| {
        let url = url.to_string();
        let importer = importer.map(String::from);
        let resolve_url = Arc::clone(&value.resolve_url);
        Box::pin(async move {
          resolve_url.await_call((url, importer).into()).await.map_err(anyhow::Error::from)
        })
      }),
      asset_inline_limit: value.asset_inline_limit.into(),
    }
  }
}
