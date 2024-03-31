use serde::Deserialize;
use std::fmt::Debug;

use crate::types::{
  binding_outputs::BindingOutputs, binding_rendered_chunk::RenderedChunk,
  js_callback::MaybeAsyncJsCallback,
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
  #[napi(ts_type = "(error: Nullable<string>) => MaybePromise<VoidNullable>")]
  pub build_end: Option<MaybeAsyncJsCallback<Option<String>, ()>>,

  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(code: string, chunk: RenderedChunk) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>"
  )]
  pub render_chunk:
    Option<MaybeAsyncJsCallback<(String, RenderedChunk), Option<BindingHookRenderChunkOutput>>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(bundle: BindingOutputs, isWrite: boolean) => MaybePromise<VoidNullable>")]
  pub generate_bundle: Option<MaybeAsyncJsCallback<(BindingOutputs, bool), ()>>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "(bundle: BindingOutputs) => MaybePromise<VoidNullable>")]
  pub write_bundle: Option<MaybeAsyncJsCallback<BindingOutputs, ()>>,
}

impl Debug for BindingPluginOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingPluginOptions").field("name", &self.name).finish_non_exhaustive()
  }
}
