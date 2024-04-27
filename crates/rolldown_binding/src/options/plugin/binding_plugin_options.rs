use napi::bindgen_prelude::FromNapiValue;
use serde::Deserialize;
use std::fmt::Debug;

use crate::types::{
  binding_module_info::BindingModuleInfo, binding_outputs::BindingOutputs,
  binding_rendered_chunk::RenderedChunk, js_callback::MaybeAsyncJsCallback,
};

use super::{
  binding_plugin_context::BindingPluginContext,
  types::{
    binding_hook_load_output::BindingHookLoadOutput,
    binding_hook_render_chunk_output::BindingHookRenderChunkOutput,
    binding_hook_resolve_id_extra_options::BindingHookResolveIdExtraOptions,
    binding_hook_resolve_id_output::BindingHookResolveIdOutput,
  },
};

/// none is parallel js plugin
pub type BindingPluginOrParallelJsPluginPlaceholder = Option<BindingPluginOptions<true>>;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingPluginOptions<const WEAK: bool = false> {
  pub name: String,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext) => MaybePromise<VoidNullable>")]
  pub build_start: Option<MaybeAsyncJsCallback<BindingPluginContext, ()>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraOptions) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>"
  )]
  pub resolve_id: Option<
    MaybeAsyncJsCallback<
      (String, Option<String>, BindingHookResolveIdExtraOptions),
      Option<BindingHookResolveIdOutput>,
    >,
  >,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>")]
  pub load: Option<MaybeAsyncJsCallback<String, Option<BindingHookLoadOutput>>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(id: string, code: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>"
  )]
  pub transform: Option<MaybeAsyncJsCallback<(String, String), Option<BindingHookLoadOutput>>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>"
  )]
  pub module_parsed: Option<MaybeAsyncJsCallback<(BindingPluginContext, BindingModuleInfo), ()>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(error: Nullable<string>) => MaybePromise<VoidNullable>")]
  pub build_end: Option<MaybeAsyncJsCallback<Option<String>, ()>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(code: string, chunk: RenderedChunk) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>"
  )]
  pub render_chunk:
    Option<MaybeAsyncJsCallback<(String, RenderedChunk), Option<BindingHookRenderChunkOutput>>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "() => void")]
  pub render_start: Option<MaybeAsyncJsCallback<(), ()>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(error: string) => void")]
  pub render_error: Option<MaybeAsyncJsCallback<String, ()>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(bundle: BindingOutputs, isWrite: boolean) => MaybePromise<VoidNullable>")]
  pub generate_bundle: Option<MaybeAsyncJsCallback<(BindingOutputs, bool), ()>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(bundle: BindingOutputs) => MaybePromise<VoidNullable>")]
  pub write_bundle: Option<MaybeAsyncJsCallback<BindingOutputs, ()>>,
}

impl<const WEAK: bool> Debug for BindingPluginOptions<WEAK> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingPluginOptions").field("name", &self.name).finish_non_exhaustive()
  }
}

impl FromNapiValue for BindingPluginOptions<true> {
  #[allow(deprecated)] // cb.unref is deprecated, fix napi_derive to support const generics
  unsafe fn from_napi_value(
    env: napi::sys::napi_env,
    napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    let mut false_options = BindingPluginOptions::<false>::from_napi_value(env, napi_val)?;
    let env = &napi::Env::from_raw(env);

    if let Some(cb) = &mut false_options.build_start {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.resolve_id {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.load {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.transform {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.module_parsed {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.build_end {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.render_chunk {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.render_start {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.render_error {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.generate_bundle {
      cb.unref(env)?;
    }
    if let Some(cb) = &mut false_options.write_bundle {
      cb.unref(env)?;
    }

    Ok(Self {
      name: false_options.name,
      build_start: false_options.build_start,
      resolve_id: false_options.resolve_id,
      load: false_options.load,
      transform: false_options.transform,
      module_parsed: false_options.module_parsed,
      build_end: false_options.build_end,
      render_chunk: false_options.render_chunk,
      render_start: false_options.render_start,
      render_error: false_options.render_error,
      generate_bundle: false_options.generate_bundle,
      write_bundle: false_options.write_bundle,
    })
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingPluginWithIndex {
  pub index: u32,
  pub plugin: BindingPluginOptions,
}
