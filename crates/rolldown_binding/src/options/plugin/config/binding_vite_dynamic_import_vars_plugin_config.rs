use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use rolldown_plugin_vite_dynamic_import_vars::{ResolverFn, ViteDynamicImportVarsPlugin};

use crate::types::{
  binding_string_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array},
  js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _},
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingViteDynamicImportVarsPluginConfig {
  pub sourcemap: Option<bool>,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  #[napi(ts_type = "(id: string, importer: string) => MaybePromise<string | undefined>")]
  pub resolver: Option<MaybeAsyncJsCallback<FnArgs<(String, String)>, Option<String>>>,
}

impl From<BindingViteDynamicImportVarsPluginConfig> for ViteDynamicImportVarsPlugin {
  fn from(value: BindingViteDynamicImportVarsPluginConfig) -> Self {
    Self {
      sourcemap: value.sourcemap.unwrap_or_default(),
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
    }
  }
}
