use napi::Either;
use serde::Deserialize;
use std::fmt::Debug;

use crate::types::{
  binding_module_info::BindingModuleInfo, binding_outputs::BindingOutputs,
  binding_rendered_chunk::RenderedChunk, js_callback::MaybeAsyncJsCallback,
};

use super::{
  binding_builtin_plugin::BindingBuiltinPlugin,
  binding_plugin_context::BindingPluginContext,
  binding_transform_context::BindingTransformPluginContext,
  types::{
    binding_hook_load_output::BindingHookLoadOutput,
    binding_hook_render_chunk_output::BindingHookRenderChunkOutput,
    binding_hook_resolve_id_extra_options::BindingHookResolveIdExtraOptions,
    binding_hook_resolve_id_output::BindingHookResolveIdOutput,
  },
};

/// none is parallel js plugin
pub type BindingPluginOrParallelJsPluginPlaceholder =
  Option<Either<BindingPluginOptions, BindingBuiltinPlugin>>;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingPluginOptions {
  pub name: String,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext) => MaybePromise<VoidNullable>")]
  pub build_start: Option<MaybeAsyncJsCallback<BindingPluginContext, ()>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraOptions) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>"
  )]
  pub resolve_id: Option<
    MaybeAsyncJsCallback<
      (BindingPluginContext, String, Option<String>, BindingHookResolveIdExtraOptions),
      Option<BindingHookResolveIdOutput>,
    >,
  >,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, specifier: string, importer: Nullable<string>) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>"
  )]
  pub resolve_dynamic_import: Option<
    MaybeAsyncJsCallback<
      (BindingPluginContext, String, Option<String>),
      Option<BindingHookResolveIdOutput>,
    >,
  >,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>"
  )]
  pub load:
    Option<MaybeAsyncJsCallback<(BindingPluginContext, String), Option<BindingHookLoadOutput>>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx:  BindingTransformPluginContext, id: string, code: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>"
  )]
  pub transform: Option<
    MaybeAsyncJsCallback<
      (BindingTransformPluginContext, String, String),
      Option<BindingHookLoadOutput>,
    >,
  >,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>"
  )]
  pub module_parsed: Option<MaybeAsyncJsCallback<(BindingPluginContext, BindingModuleInfo), ()>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, error: Nullable<string>) => MaybePromise<VoidNullable>"
  )]
  pub build_end: Option<MaybeAsyncJsCallback<(BindingPluginContext, Option<String>), ()>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, code: string, chunk: RenderedChunk) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>"
  )]
  pub render_chunk: Option<
    MaybeAsyncJsCallback<
      (BindingPluginContext, String, RenderedChunk),
      Option<BindingHookRenderChunkOutput>,
    >,
  >,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, chunk: RenderedChunk) => MaybePromise<void | string>"
  )]
  pub augment_chunk_hash:
    Option<MaybeAsyncJsCallback<(BindingPluginContext, RenderedChunk), Option<String>>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext) => void")]
  pub render_start: Option<MaybeAsyncJsCallback<BindingPluginContext, ()>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext, error: string) => void")]
  pub render_error: Option<MaybeAsyncJsCallback<(BindingPluginContext, String), ()>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, bundle: BindingOutputs, isWrite: boolean) => MaybePromise<VoidNullable>"
  )]
  pub generate_bundle:
    Option<MaybeAsyncJsCallback<(BindingPluginContext, BindingOutputs, bool), ()>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, bundle: BindingOutputs) => MaybePromise<VoidNullable>"
  )]
  pub write_bundle: Option<MaybeAsyncJsCallback<(BindingPluginContext, BindingOutputs), ()>>,
}

impl Debug for BindingPluginOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingPluginOptions").field("name", &self.name).finish_non_exhaustive()
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingPluginWithIndex {
  pub index: u32,
  pub plugin: BindingPluginOptions,
}
