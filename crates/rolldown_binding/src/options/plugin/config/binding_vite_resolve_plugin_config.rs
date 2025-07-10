use derive_more::Debug;
use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use rolldown_plugin_vite_resolve::{
  FinalizeBareSpecifierCallback, FinalizeOtherSpecifiersCallback, ViteResolveOptions,
  ViteResolveResolveOptions,
};

use crate::{
  options::plugin::types::{
    binding_hook_resolve_id_output::BindingHookResolveIdOutput,
    binding_limited_boolean::BindingTrueValue,
  },
  types::{
    binding_string_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array},
    js_callback::{
      JsCallback, JsCallbackExt as _, MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _,
    },
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
  #[debug("{}", if finalize_other_specifiers.is_some() { "Some(<finalize_bare_specifier>)" } else { "None" })]
  #[napi(
    ts_type = "(id: string, importer: string | undefined, scan: boolean) => MaybePromise<BindingHookResolveIdOutput | undefined>"
  )]
  pub finalize_bare_specifier: Option<
    MaybeAsyncJsCallback<
      FnArgs<(String, Option<String>, bool)>,
      Option<BindingHookResolveIdOutput>,
    >,
  >,
  #[debug("{}", if finalize_other_specifiers.is_some() { "Some(<finalize_other_specifiers>)" } else { "None" })]
  #[napi(ts_type = "(resolvedId: string, rawId: string) => VoidNullable<string>")]
  pub finalize_other_specifiers: Option<JsCallback<FnArgs<(String, String)>, Option<String>>>,
  #[debug("Some(<resolve_subpath_imports>)")]
  #[napi(
    ts_type = "(id: string, importer: string, isRequire: boolean, scan: boolean) => VoidNullable<string>"
  )]
  pub resolve_subpath_imports:
    JsCallback<FnArgs<(String, Option<String>, bool, bool)>, Option<String>>,
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
        |finalize_bare_specifier| -> Arc<FinalizeBareSpecifierCallback> {
          Arc::new(move |id: &str, importer: Option<&str>, scan: bool| {
            let id = id.to_owned();
            let importer = importer.map(ToString::to_string);
            let finalizer_fn = Arc::clone(&finalize_bare_specifier);
            Box::pin(async move {
              finalizer_fn
                .await_call((id, importer, scan).into())
                .await
                .map(|v| v.and_then(|v| v.try_into().ok()))
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
      resolve_subpath_imports: Arc::new(
        move |id: &str, importer: Option<&str>, is_require: bool, scan: bool| {
          let resolve_fn = Arc::clone(&value.resolve_subpath_imports);
          let id = id.to_owned();
          let importer = importer.map(std::string::ToString::to_string);
          Box::pin(async move {
            resolve_fn
              .invoke_async((id, importer, is_require, scan).into())
              .await
              .map_err(anyhow::Error::from)
          })
        },
      ),
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingViteResolvePluginResolveOptions {
  pub is_build: bool,
  pub is_production: bool,
  pub as_src: bool,
  pub prefer_relative: bool,
  pub is_require: Option<bool>,
  pub root: String,
  pub scan: bool,

  pub main_fields: Vec<String>,
  pub conditions: Vec<String>,
  pub external_conditions: Vec<String>,
  pub extensions: Vec<String>,
  pub try_index: bool,
  pub try_prefix: Option<String>,
  pub preserve_symlinks: bool,
  pub tsconfig_paths: bool,
}

impl From<BindingViteResolvePluginResolveOptions> for ViteResolveResolveOptions {
  fn from(value: BindingViteResolvePluginResolveOptions) -> Self {
    Self {
      is_build: value.is_build,
      is_production: value.is_production,
      as_src: value.as_src,
      prefer_relative: value.prefer_relative,
      is_require: value.is_require,
      root: value.root,
      scan: value.scan,

      main_fields: value.main_fields,
      conditions: value.conditions,
      external_conditions: value.external_conditions,
      extensions: value.extensions,
      try_index: value.try_index,
      try_prefix: value.try_prefix,
      preserve_symlinks: value.preserve_symlinks,
      tsconfig_paths: value.tsconfig_paths,
    }
  }
}
