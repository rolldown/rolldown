use std::sync::Arc;

use napi::{
  Either,
  bindgen_prelude::{FnArgs, FromNapiValue},
  sys,
};
use rolldown_plugin_utils::{RenderBuiltUrl, RenderBuiltUrlConfig, RenderBuiltUrlRet};

use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _};

#[napi_derive::napi(object, object_from_js = false)]
pub struct BindingRenderBuiltUrlConfig {
  pub ssr: bool,
  #[napi(ts_type = "'asset' | 'public'")]
  pub r#type: String,
  pub host_id: String,
  #[napi(ts_type = "'js' | 'css' | 'html'")]
  pub host_type: String,
}

impl From<&RenderBuiltUrlConfig<'_>> for BindingRenderBuiltUrlConfig {
  fn from(value: &RenderBuiltUrlConfig) -> Self {
    Self {
      ssr: value.is_ssr,
      r#type: value.r#type.to_string(),
      host_id: value.host_id.to_string(),
      host_type: value.host_type.to_string(),
    }
  }
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingRenderBuiltUrlRet {
  pub relative: Option<bool>,
  pub runtime: Option<String>,
}

impl From<BindingRenderBuiltUrlRet> for RenderBuiltUrlRet {
  fn from(value: BindingRenderBuiltUrlRet) -> Self {
    Self { relative: value.relative, runtime: value.runtime }
  }
}

pub struct BindingRenderBuiltUrl(Arc<RenderBuiltUrl>);

impl FromNapiValue for BindingRenderBuiltUrl {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    unsafe {
      let render_built_url = MaybeAsyncJsCallback::<
        FnArgs<(String, BindingRenderBuiltUrlConfig)>,
        Option<Either<String, BindingRenderBuiltUrlRet>>,
      >::from_napi_value(env, napi_val)?;
      Ok(Self(Arc::new(move |filename: &str, config: &RenderBuiltUrlConfig| {
        let render_built_url = Arc::clone(&render_built_url);
        let filename = filename.to_string();
        let config = config.into();
        Box::pin(async move {
          render_built_url
            .await_call((filename, config).into())
            .await
            .map(|v| {
              v.map(|v| match v {
                Either::A(v) => itertools::Either::Left(v),
                Either::B(v) => itertools::Either::Right(v.into()),
              })
            })
            .map_err(anyhow::Error::from)
        })
      })))
    }
  }
}

impl From<BindingRenderBuiltUrl> for Arc<RenderBuiltUrl> {
  fn from(value: BindingRenderBuiltUrl) -> Self {
    value.0
  }
}
