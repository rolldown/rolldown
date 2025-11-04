use std::sync::Arc;

use napi::{
  Either,
  bindgen_prelude::{FnArgs, FromNapiValue},
  sys,
};
use rolldown_plugin_utils::{ModulePreload, ModulePreloadOptions, ResolveDependenciesFn};

use crate::types::js_callback::{JsCallback, JsCallbackExt as _};

#[napi_derive::napi(object, object_from_js = true)]
pub struct BindingResolveDependenciesContext {
  pub host_id: String,
  pub host_type: String,
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingModulePreloadOptions {
  pub polyfill: bool,
  #[napi(
    ts_type = "(filename: string, deps: string[], context: { hostId: string, hostType: 'html' | 'js' }) => string[]"
  )]
  pub resolve_dependencies: Option<
    JsCallback<FnArgs<(String, Vec<String>, BindingResolveDependenciesContext)>, Vec<String>>,
  >,
}

pub struct BindingModulePreload(ModulePreload);

impl From<BindingModulePreload> for ModulePreload {
  fn from(value: BindingModulePreload) -> Self {
    value.0
  }
}

impl FromNapiValue for BindingModulePreload {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    unsafe {
      let module_preload =
        Either::<bool, BindingModulePreloadOptions>::from_napi_value(env, napi_val)?;
      Ok(match module_preload {
        Either::A(v) => {
          if v {
            return Err(napi::Error::from_reason(
              "The `modulePreload` shouldn't be `true`".to_string(),
            ));
          }
          Self(ModulePreload::False)
        }
        Either::B(v) => {
          let resolve_dependencies =
            v.resolve_dependencies.map(|resolve_dependencies| -> Arc<ResolveDependenciesFn> {
              Arc::new(move |filename: &str, deps: Vec<String>, host_id: &str, host_type: &str| {
                let filename = filename.to_string();
                let context = BindingResolveDependenciesContext {
                  host_id: host_id.to_string(),
                  host_type: host_type.to_string(),
                };
                let resolve_dependencies = Arc::clone(&resolve_dependencies);
                Box::pin(async move {
                  resolve_dependencies
                    .invoke_async((filename, deps, context).into())
                    .await
                    .map_err(anyhow::Error::from)
                })
              })
            });
          Self(ModulePreload::Options(ModulePreloadOptions {
            polyfill: v.polyfill,
            resolve_dependencies,
          }))
        }
      })
    }
  }
}
