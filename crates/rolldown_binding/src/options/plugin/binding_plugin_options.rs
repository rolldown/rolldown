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
#[derive(Default)]
pub struct HookOption {
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
  pub sequential: Option<bool>,
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct BuildStartHookOption {
  #[napi(ts_type = "(ctx: BindingPluginContext) => MaybePromise<VoidNullable>")]
  pub handler: MaybeAsyncJsCallback<BindingPluginContext, ()>,
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
  pub sequential: Option<bool>,
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct ResolveIdHookOption {
  #[napi(
    ts_type = "(specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraOptions) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>"
  )]
  pub handler: MaybeAsyncJsCallback<
    (String, Option<String>, BindingHookResolveIdExtraOptions),
    Option<BindingHookResolveIdOutput>,
  >,
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
}
#[napi_derive::napi(object, object_to_js = false)]
pub struct LoadOption {
  #[napi(ts_type = "(id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>")]
  pub handler: MaybeAsyncJsCallback<String, Option<BindingHookLoadOutput>>,
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
}
#[napi_derive::napi(object, object_to_js = false)]
pub struct TransformOption {
  #[napi(
    ts_type = "(id: string, code: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>"
  )]
  pub handler: MaybeAsyncJsCallback<(String, String), Option<BindingHookLoadOutput>>,
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
}
#[napi_derive::napi(object, object_to_js = false)]
pub struct BuildEndHookOption {
  #[napi(ts_type = "(error: Nullable<string>) => MaybePromise<VoidNullable>")]
  pub handler: MaybeAsyncJsCallback<Option<String>, ()>,
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
  pub sequential: Option<bool>,
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct RenderChunkOption {
  #[napi(
    ts_type = "(code: string, chunk: RenderedChunk) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>"
  )]
  pub handler: MaybeAsyncJsCallback<(String, RenderedChunk), Option<BindingHookRenderChunkOutput>>,
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
}
#[napi_derive::napi(object, object_to_js = false)]
pub struct GenerateBundleOption {
  #[napi(ts_type = "(bundle: BindingOutputs, isWrite: boolean) => MaybePromise<VoidNullable>")]
  pub handler: MaybeAsyncJsCallback<(BindingOutputs, bool), ()>,
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct WriteBundleOption {
  #[napi(ts_type = "(bundle: BindingOutputs) => MaybePromise<VoidNullable>")]
  pub handler: MaybeAsyncJsCallback<BindingOutputs, ()>,
  #[napi(ts_type = "'pre'|'post'|null")]
  pub order: Option<String>,
  pub sequential: Option<bool>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingPluginOptions {
  pub name: String,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "BuildStartHookOption")]
  pub build_start: Option<BuildStartHookOption>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "ResolveIdHookOption")]
  pub resolve_id: Option<ResolveIdHookOption>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "LoadOption")]
  pub load: Option<LoadOption>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "TransformOption")]
  pub transform: Option<TransformOption>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "BuildEndHookOption")]
  pub build_end: Option<BuildEndHookOption>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "RenderChunkOption")]
  pub render_chunk: Option<RenderChunkOption>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "GenerateBundleOption")]
  pub generate_bundle: Option<GenerateBundleOption>,

  #[serde(skip_deserializing)]
  #[napi(ts_type = "WriteBundleOption")]
  pub write_bundle: Option<WriteBundleOption>,
}

impl Debug for BindingPluginOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingPluginOptions").field("name", &self.name).finish_non_exhaustive()
  }
}
