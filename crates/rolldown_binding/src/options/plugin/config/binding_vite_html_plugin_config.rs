use std::sync::Arc;

use napi::{Either, bindgen_prelude::FnArgs};
use rolldown_plugin_utils::{RenderBuiltUrl, RenderBuiltUrlConfig};
use rolldown_plugin_vite_html::{ResolveDependenciesEither, ViteHtmlPlugin};
use rolldown_utils::dashmap::FxDashMap;

use crate::{
  options::plugin::{
    config::binding_asset_plugin_config::{BindingRenderBuiltUrlConfig, BindingRenderBuiltUrlRet},
    types::binding_asset_inline_limit::BindingAssetInlineLimit,
  },
  types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _},
};

#[expect(clippy::struct_excessive_bools)]
#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteHtmlPluginConfig {
  pub is_lib: bool,
  pub is_ssr: bool,
  pub url_base: String,
  pub public_dir: String,
  pub decoded_base: String,
  pub css_code_split: bool,
  pub module_preload_polyfill: bool,
  #[napi(ts_type = "number | ((file: string, content: Buffer) => boolean | undefined)")]
  pub asset_inline_limit: BindingAssetInlineLimit,
  #[napi(
    ts_type = "(filename: string, type: BindingRenderBuiltUrlConfig) => MaybePromise<VoidNullable<string | BindingRenderBuiltUrlRet>>"
  )]
  pub render_built_url: Option<
    MaybeAsyncJsCallback<
      FnArgs<(String, BindingRenderBuiltUrlConfig)>,
      Option<Either<String, BindingRenderBuiltUrlRet>>,
    >,
  >,
  #[napi(
    ts_type = "boolean | ((filename: string, dependencies: string[], context: { hostId: string, hostType: 'html' | 'js' }) => Promise<string[]>)"
  )]
  pub resolve_dependencies: Option<
    Either<
      bool,
      MaybeAsyncJsCallback<
        FnArgs<(String, Vec<String>, BindingResolveDependenciesContext)>,
        Vec<String>,
      >,
    >,
  >,
}

#[napi_derive::napi(object, object_from_js = true)]
pub struct BindingResolveDependenciesContext {
  pub host_id: String,
  pub host_type: String,
}

impl From<BindingViteHtmlPluginConfig> for ViteHtmlPlugin {
  fn from(value: BindingViteHtmlPluginConfig) -> Self {
    let render_built_url = value.render_built_url.map(|render_built_url| -> Arc<RenderBuiltUrl> {
      Arc::new(move |filename: &str, config: &RenderBuiltUrlConfig| {
        let render_built_url = Arc::clone(&render_built_url);
        let filename = filename.to_string();
        let config = config.into();
        Box::pin(async move {
          render_built_url.await_call((filename, config).into()).await.map(|v| {
            v.map(|v| match v {
              Either::A(v) => itertools::Either::Left(v),
              Either::B(v) => itertools::Either::Right(v.into()),
            })
          })
        })
      })
    });

    let resolve_dependencies = match value.resolve_dependencies {
      Some(Either::A(true)) => Some(ResolveDependenciesEither::True),
      Some(Either::B(resolve_dependencies)) => Some(ResolveDependenciesEither::Fn(Arc::new(
        move |filename: &str, deps: Vec<String>, host_id: &str, host_type: &str| {
          let filename = filename.to_string();
          let context = BindingResolveDependenciesContext {
            host_id: host_id.to_string(),
            host_type: host_type.to_string(),
          };

          let resolve_dependencies = Arc::clone(&resolve_dependencies);
          Box::pin(async move {
            resolve_dependencies.await_call((filename, deps, context).into()).await
          })
        },
      ))),
      _ => None,
    };

    Self {
      is_lib: value.is_lib,
      is_ssr: value.is_ssr,
      url_base: value.url_base,
      public_dir: value.public_dir,
      decoded_base: value.decoded_base,
      css_code_split: value.css_code_split,
      module_preload_polyfill: value.module_preload_polyfill,
      asset_inline_limit: value.asset_inline_limit.into(),
      render_built_url,
      resolve_dependencies,
      html_result_map: FxDashMap::default(),
    }
  }
}
