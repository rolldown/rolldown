use std::collections::HashMap;

use derivative::Derivative;
use napi::{
  bindgen_prelude::{Either, Either3, Promise},
  threadsafe_function::{ThreadsafeFunction, UnknownReturnValue},
};
use rolldown_error::BuildError;
use serde::Deserialize;

use crate::types::{
  binding_outputs::BindingOutputs, binding_rendered_module::BindingRenderedModule,
  js_async_callback::JsAsyncCallback,
};

use super::binding_plugin_context::BindingPluginContext;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct PluginOptions {
  pub name: String,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: BindingPluginContext) => Promise<void>")]
  pub build_start: Option<JsAsyncCallback<BindingPluginContext, ()>>,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(specifier: string, importer?: string, options?: HookResolveIdArgsOptions) => Promise<undefined | ResolveIdResult>"
  )]
  pub resolve_id: Option<
    ThreadsafeFunction<
      (String, Option<String>, Option<HookResolveIdArgsOptions>),
      Either3<Promise<Option<ResolveIdResult>>, Option<ResolveIdResult>, UnknownReturnValue>,
      false,
    >,
  >,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(id: string) => Promise<undefined | SourceResult>")]
  pub load: Option<
    ThreadsafeFunction<
      String,
      Either3<Promise<Option<SourceResult>>, Option<SourceResult>, UnknownReturnValue>,
      false,
    >,
  >,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(id: string, code: string) => Promise<undefined | SourceResult>")]
  pub transform: Option<
    ThreadsafeFunction<
      (String, String),
      Either3<Promise<Option<SourceResult>>, Option<SourceResult>, UnknownReturnValue>,
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
    ts_type = "(code: string, chunk: RenderedChunk) => Promise<undefined | HookRenderChunkOutput>"
  )]
  pub render_chunk: Option<
    ThreadsafeFunction<
      (String, RenderedChunk),
      Either3<
        Promise<Option<HookRenderChunkOutput>>,
        Option<HookRenderChunkOutput>,
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

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct HookResolveIdArgsOptions {
  pub is_entry: bool,
  pub kind: String,
}

impl From<rolldown_plugin::HookResolveIdExtraOptions> for HookResolveIdArgsOptions {
  fn from(value: rolldown_plugin::HookResolveIdExtraOptions) -> Self {
    Self { is_entry: value.is_entry, kind: value.kind.to_string() }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct ResolveIdResult {
  pub id: String,
  pub external: Option<bool>,
}

impl From<ResolveIdResult> for rolldown_plugin::HookResolveIdOutput {
  fn from(value: ResolveIdResult) -> Self {
    Self { id: value.id, external: value.external }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct SourceResult {
  pub code: String,
  pub map: Option<String>,
}

impl TryFrom<SourceResult> for rolldown_plugin::HookLoadOutput {
  type Error = BuildError;

  fn try_from(value: SourceResult) -> Result<Self, Self::Error> {
    Ok(rolldown_plugin::HookLoadOutput {
      code: value.code,
      map: value
        .map
        .map(|content| {
          rolldown_sourcemap::SourceMap::from_slice(content.as_bytes())
            .map_err(|e| BuildError::sourcemap_error(e.to_string()))
        })
        .transpose()?,
    })
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct HookRenderChunkOutput {
  pub code: String,
}

impl From<HookRenderChunkOutput> for rolldown_plugin::HookRenderChunkOutput {
  fn from(value: HookRenderChunkOutput) -> Self {
    Self { code: value.code }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derive(Debug)]
pub struct PreRenderedChunk {
  // pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
}

impl From<rolldown::PreRenderedChunk> for PreRenderedChunk {
  fn from(value: rolldown::PreRenderedChunk) -> Self {
    Self {
      is_entry: value.is_entry,
      is_dynamic_entry: value.is_dynamic_entry,
      facade_module_id: value.facade_module_id,
      module_ids: value.module_ids,
      exports: value.exports,
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derive(Debug)]
pub struct RenderedChunk {
  // PreRenderedChunk
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: String,
  #[serde(skip)]
  pub modules: HashMap<String, BindingRenderedModule>,
}

impl From<rolldown_common::RenderedChunk> for RenderedChunk {
  fn from(value: rolldown_common::RenderedChunk) -> Self {
    Self {
      is_entry: value.is_entry,
      is_dynamic_entry: value.is_dynamic_entry,
      facade_module_id: value.facade_module_id,
      module_ids: value.module_ids,
      exports: value.exports,
      file_name: value.file_name,
      modules: value.modules.into_iter().map(|(key, value)| (key, value.into())).collect(),
    }
  }
}
