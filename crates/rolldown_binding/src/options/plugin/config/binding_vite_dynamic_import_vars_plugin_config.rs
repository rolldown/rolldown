use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use rolldown_plugin_vite_dynamic_import_vars::{
  ResolverFn, ViteDynamicImportVarsPlugin, ViteDynamicImportVarsPluginV2Config,
};

use crate::types::{
  binding_string_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array},
  js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _},
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingViteDynamicImportVarsPluginV2Config {
  pub sourcemap: bool,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingViteDynamicImportVarsPluginConfig {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  #[napi(ts_type = "(id: string, importer: string) => MaybePromise<string | undefined>")]
  pub resolver: Option<MaybeAsyncJsCallback<FnArgs<(String, String)>, Option<String>>>,
  pub is_v2: Option<BindingViteDynamicImportVarsPluginV2Config>,
}

impl From<BindingViteDynamicImportVarsPluginConfig> for ViteDynamicImportVarsPlugin {
  fn from(value: BindingViteDynamicImportVarsPluginConfig) -> Self {
    Self {
      include: value.include.map(bindingify_string_or_regex_array).unwrap_or_default(),
      exclude: value.exclude.map(bindingify_string_or_regex_array).unwrap_or_default(),
      resolver: value.resolver.map(|resolver| -> Arc<ResolverFn> {
        Arc::new(move |id: String, importer: String| {
          let resolver = Arc::clone(&resolver);
          Box::pin(async move {
            resolver.await_call((id, importer).into()).await.map_err(anyhow::Error::from)
          })
        })
      }),
      is_v2: value
        .is_v2
        .map(|v2_config| ViteDynamicImportVarsPluginV2Config { sourcemap: v2_config.sourcemap }),
    }
  }
}
