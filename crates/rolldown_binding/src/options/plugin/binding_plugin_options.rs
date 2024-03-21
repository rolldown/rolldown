use derivative::Derivative;
use napi::{
  bindgen_prelude::{Either, Either3, Promise},
  threadsafe_function::{ThreadsafeFunction, UnknownReturnValue},
};
use serde::Deserialize;

use crate::types::{
  binding_outputs::BindingOutputs, binding_rendered_chunk::RenderedChunk,
  js_async_callback::JsAsyncCallback,
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
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingPluginOptions {
  pub name: String,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext) => MaybePromise<void>")]
  pub build_start: Option<JsAsyncCallback<BindingPluginContext, ()>>,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(specifier: string, importer?: string, options?: BindingHookResolveIdExtraOptions) => Promise<undefined | BindingHookResolveIdOutput>"
  )]
  pub resolve_id: Option<
    ThreadsafeFunction<
      (String, Option<String>, Option<BindingHookResolveIdExtraOptions>),
      Either3<
        Promise<Option<BindingHookResolveIdOutput>>,
        Option<BindingHookResolveIdOutput>,
        UnknownReturnValue,
      >,
      false,
    >,
  >,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(id: string) => Promise<undefined | BindingHookLoadOutput>")]
  pub load: Option<
    ThreadsafeFunction<
      String,
      Either3<
        Promise<Option<BindingHookLoadOutput>>,
        Option<BindingHookLoadOutput>,
        UnknownReturnValue,
      >,
      false,
    >,
  >,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(id: string, code: string) => Promise<undefined | BindingHookLoadOutput>")]
  pub transform: Option<
    ThreadsafeFunction<
      (String, String),
      Either3<
        Promise<Option<BindingHookLoadOutput>>,
        Option<BindingHookLoadOutput>,
        UnknownReturnValue,
      >,
      false,
    >,
  >,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(error?: string) => Promise<void>")]
  pub build_end:
    Option<ThreadsafeFunction<Option<String>, Either<Promise<()>, UnknownReturnValue>, false>>,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(code: string, chunk: RenderedChunk) => Promise<undefined | BindingHookRenderChunkOutput>"
  )]
  pub render_chunk: Option<
    ThreadsafeFunction<
      (String, RenderedChunk),
      Either3<
        Promise<Option<BindingHookRenderChunkOutput>>,
        Option<BindingHookRenderChunkOutput>,
        UnknownReturnValue,
      >,
      false,
    >,
  >,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(bundle: Outputs, isWrite: boolean) => Promise<void>")]
  pub generate_bundle: Option<
    ThreadsafeFunction<(BindingOutputs, bool), Either<Promise<()>, UnknownReturnValue>, false>,
  >,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(bundle: Outputs) => Promise<void>")]
  pub write_bundle:
    Option<ThreadsafeFunction<BindingOutputs, Either<Promise<()>, UnknownReturnValue>, false>>,
}
