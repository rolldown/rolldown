use napi::bindgen_prelude::Either;
use serde::Deserialize;
use std::fmt::Debug;

use crate::types::{
  binding_module_info::BindingModuleInfo, binding_outputs::BindingOutputs,
  binding_rendered_chunk::RenderedChunk, js_callback::MaybeAsyncJsCallback,
};

use super::{
  binding_builtin_plugin::BindingBuiltinPlugin,
  binding_plugin_context::BindingPluginContext,
  binding_plugin_hook_meta::BindingPluginHookMeta,
  binding_transform_context::BindingTransformPluginContext,
  types::{
    binding_hook_load_output::BindingHookLoadOutput,
    binding_hook_render_chunk_output::BindingHookRenderChunkOutput,
    binding_hook_resolve_id_extra_args::BindingHookResolveIdExtraArgs,
    binding_hook_resolve_id_output::BindingHookResolveIdOutput,
    binding_hook_transform_output::BindingHookTransformOutput,
    binding_plugin_transform_extra_args::BindingTransformHookExtraArgs,
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
  pub build_start_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraArgs) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>"
  )]
  pub resolve_id: Option<
    MaybeAsyncJsCallback<
      (BindingPluginContext, String, Option<String>, BindingHookResolveIdExtraArgs),
      Option<BindingHookResolveIdOutput>,
    >,
  >,
  pub resolve_id_meta: Option<BindingPluginHookMeta>,

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
  pub resolve_dynamic_import_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>"
  )]
  pub load:
    Option<MaybeAsyncJsCallback<(BindingPluginContext, String), Option<BindingHookLoadOutput>>>,
  pub load_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx:  BindingTransformPluginContext, id: string, code: string, module_type: BindingTransformHookExtraArgs) => MaybePromise<VoidNullable<BindingHookTransformOutput>>"
  )]
  pub transform: Option<
    MaybeAsyncJsCallback<
      (BindingTransformPluginContext, String, String, BindingTransformHookExtraArgs),
      Option<BindingHookTransformOutput>,
    >,
  >,
  pub transform_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>"
  )]
  pub module_parsed: Option<MaybeAsyncJsCallback<(BindingPluginContext, BindingModuleInfo), ()>>,
  pub module_parsed_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, error: Nullable<string>) => MaybePromise<VoidNullable>"
  )]
  pub build_end: Option<MaybeAsyncJsCallback<(BindingPluginContext, Option<String>), ()>>,
  pub build_end_meta: Option<BindingPluginHookMeta>,

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
  pub render_chunk_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, chunk: RenderedChunk) => MaybePromise<void | string>"
  )]
  pub augment_chunk_hash:
    Option<MaybeAsyncJsCallback<(BindingPluginContext, RenderedChunk), Option<String>>>,
  pub augment_chunk_hash_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext) => void")]
  pub render_start: Option<MaybeAsyncJsCallback<BindingPluginContext, ()>>,
  pub render_start_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext, error: string) => void")]
  pub render_error: Option<MaybeAsyncJsCallback<(BindingPluginContext, String), ()>>,
  pub render_error_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, bundle: BindingOutputs, isWrite: boolean) => MaybePromise<VoidNullable>"
  )]
  pub generate_bundle:
    Option<MaybeAsyncJsCallback<(BindingPluginContext, BindingOutputs, bool), ()>>,
  pub generate_bundle_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: BindingPluginContext, bundle: BindingOutputs) => MaybePromise<VoidNullable>"
  )]
  pub write_bundle: Option<MaybeAsyncJsCallback<(BindingPluginContext, BindingOutputs), ()>>,
  pub write_bundle_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext, chunk: RenderedChunk) => void")]
  pub banner: Option<MaybeAsyncJsCallback<(BindingPluginContext, RenderedChunk), Option<String>>>,
  pub banner_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext, chunk: RenderedChunk) => void")]
  pub footer: Option<MaybeAsyncJsCallback<(BindingPluginContext, RenderedChunk), Option<String>>>,
  pub footer_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext, chunk: RenderedChunk) => void")]
  pub intro: Option<MaybeAsyncJsCallback<(BindingPluginContext, RenderedChunk), Option<String>>>,
  pub intro_meta: Option<BindingPluginHookMeta>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext, chunk: RenderedChunk) => void")]
  pub outro: Option<MaybeAsyncJsCallback<(BindingPluginContext, RenderedChunk), Option<String>>>,
  pub outro_meta: Option<BindingPluginHookMeta>,
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
