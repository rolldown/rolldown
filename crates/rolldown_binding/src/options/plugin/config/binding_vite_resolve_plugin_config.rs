use derive_more::Debug;
use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use rolldown_plugin_vite_resolve::{
  FinalizeBareSpecifierCallback, FinalizeOtherSpecifiersCallback, ViteResolveOptions,
};

use crate::{
  options::plugin::{
    binding_builtin_plugin::BindingViteResolvePluginResolveOptions,
    types::binding_limited_boolean::BindingTrueValue,
  },
  types::{
    binding_string_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array},
    js_callback::{JsCallback, JsCallbackExt as _},
  },
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingViteResolvePluginConfig {
  pub resolve_options: BindingViteResolvePluginResolveOptions,
  pub environment_consumer: String,
  pub environment_name: String,
  pub builtins: Vec<BindingStringOrRegex>,
  #[napi(ts_type = "true | string[]")]
  pub external: napi::Either<BindingTrueValue, Vec<String>>,
  #[napi(ts_type = "true | Array<string | RegExp>")]
  pub no_external: napi::Either<BindingTrueValue, Vec<BindingStringOrRegex>>,
  pub dedupe: Vec<String>,
  #[debug("{}", if finalize_bare_specifier.is_some() { "Some(<finalize_bare_specifier>)" } else { "None" })]
  #[napi(
    ts_type = "(resolvedId: string, rawId: string, importer: string | null | undefined) => VoidNullable<string>"
  )]
  pub finalize_bare_specifier:
    Option<JsCallback<FnArgs<(String, String, Option<String>)>, Option<String>>>,
  #[debug("{}", if finalize_bare_specifier.is_some() { "Some(<finalize_other_specifiers>)" } else { "None" })]
  #[napi(ts_type = "(resolvedId: string, rawId: string) => VoidNullable<string>")]
  pub finalize_other_specifiers: Option<JsCallback<FnArgs<(String, String)>, Option<String>>>,
}

impl From<BindingViteResolvePluginConfig> for ViteResolveOptions {
  fn from(value: BindingViteResolvePluginConfig) -> Self {
    let external = match value.external {
      napi::Either::A(_) => rolldown_plugin_vite_resolve::ResolveOptionsExternal::True,
      napi::Either::B(v) => rolldown_plugin_vite_resolve::ResolveOptionsExternal::Vec(v),
    };
    let no_external = match value.no_external {
      napi::Either::A(_) => rolldown_plugin_vite_resolve::ResolveOptionsNoExternal::new_true(),
      napi::Either::B(v) => rolldown_plugin_vite_resolve::ResolveOptionsNoExternal::new_vec(
        bindingify_string_or_regex_array(v),
      ),
    };

    Self {
      resolve_options: value.resolve_options.into(),
      environment_consumer: value.environment_consumer,
      environment_name: value.environment_name,
      builtins: bindingify_string_or_regex_array(value.builtins),
      external,
      no_external,
      dedupe: value.dedupe,
      finalize_bare_specifier: value.finalize_bare_specifier.map(
        |finalizer_fn| -> Arc<FinalizeBareSpecifierCallback> {
          Arc::new(move |resolved_id: &str, raw_id: &str, importer: Option<&str>| {
            let finalizer_fn = Arc::clone(&finalizer_fn);
            let resolved_id = resolved_id.to_owned();
            let raw_id = raw_id.to_owned();
            let importer = importer.map(ToString::to_string);
            Box::pin(async move {
              finalizer_fn
                .invoke_async((resolved_id, raw_id, importer).into())
                .await
                .map_err(anyhow::Error::from)
            })
          })
        },
      ),
      finalize_other_specifiers: value.finalize_other_specifiers.map(
        |finalizer_fn| -> Arc<FinalizeOtherSpecifiersCallback> {
          Arc::new(move |resolved_id: &str, raw_id: &str| {
            let finalizer_fn = Arc::clone(&finalizer_fn);
            let resolved_id = resolved_id.to_owned();
            let raw_id = raw_id.to_owned();
            Box::pin(async move {
              finalizer_fn
                .invoke_async((resolved_id, raw_id).into())
                .await
                .map_err(anyhow::Error::from)
            })
          })
        },
      ),
    }
  }
}
