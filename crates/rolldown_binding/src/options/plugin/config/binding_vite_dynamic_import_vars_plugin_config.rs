use std::sync::Arc;

use napi::{
  Status, ValueType,
  bindgen_prelude::{FnArgs, FromNapiValue, TypeName, ValidateNapiValue},
  sys,
};
use rolldown_plugin_vite_dynamic_import_vars::{ResolverFn, ViteDynamicImportVarsPlugin};

use crate::async_runtime::get_runtime_capabilities;
use crate::types::{
  binding_string_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array},
  js_callback::{JsCallbackResultExt as _, MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _},
};

type JsResolver = MaybeAsyncJsCallback<FnArgs<(String, String)>, Option<String>>;

pub struct BindingViteDynamicImportVarsResolver(JsResolver);

impl TypeName for BindingViteDynamicImportVarsResolver {
  fn type_name() -> &'static str {
    "Function"
  }

  fn value_type() -> ValueType {
    ValueType::Function
  }
}

impl ValidateNapiValue for BindingViteDynamicImportVarsResolver {}

impl FromNapiValue for BindingViteDynamicImportVarsResolver {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    let runtime = get_runtime_capabilities();
    if !runtime.threads {
      return Err(napi::Error::new(
        Status::WouldDeadlock,
        format!(
          "viteDynamicImportVarsPlugin()'s resolver option is not supported by Rolldown's \
           CurrentThread runtime on the {} target. Use a MultiThread runtime or omit the resolver \
           option.",
          runtime.target
        ),
      ));
    }

    Ok(Self(unsafe { JsResolver::from_napi_value(env, napi_val)? }))
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingViteDynamicImportVarsPluginConfig {
  pub sourcemap: Option<bool>,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  #[napi(ts_type = "(id: string, importer: string) => MaybePromise<string | undefined>")]
  pub resolver: Option<BindingViteDynamicImportVarsResolver>,
}

impl From<BindingViteDynamicImportVarsPluginConfig> for ViteDynamicImportVarsPlugin {
  fn from(value: BindingViteDynamicImportVarsPluginConfig) -> Self {
    Self {
      sourcemap: value.sourcemap.unwrap_or_default(),
      include: value.include.map(bindingify_string_or_regex_array).unwrap_or_default(),
      exclude: value.exclude.map(bindingify_string_or_regex_array).unwrap_or_default(),
      resolver: value.resolver.map(|resolver| -> Arc<ResolverFn> {
        let resolver = resolver.0;
        Arc::new(move |id: String, importer: String| {
          let resolver = Arc::clone(&resolver);
          Box::pin(async move {
            resolver
              .await_call((id, importer).into())
              .await
              .context("viteDynamicImportVars resolver option")
              .map_err(anyhow::Error::from)
          })
        })
      }),
    }
  }
}
