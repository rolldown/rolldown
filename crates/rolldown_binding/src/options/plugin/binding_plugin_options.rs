use itertools::Itertools;
use napi::bindgen_prelude::{Either, FnArgs};
use rolldown_utils::filter_expression::{self, FilterExprKind};
use std::fmt::Debug;

use crate::types::{
  binding_module_info::BindingModuleInfo,
  binding_normalized_options::BindingNormalizedOptions,
  binding_outputs::{BindingError, BindingOutputs, JsChangedOutputs},
  binding_rendered_chunk::BindingRenderedChunk,
  js_callback::MaybeAsyncJsCallback,
};

use super::{
  binding_builtin_plugin::BindingBuiltinPlugin,
  binding_plugin_context::BindingPluginContext,
  binding_plugin_hook_meta::BindingPluginHookMeta,
  binding_transform_context::BindingTransformPluginContext,
  types::{
    binding_filter_expression::normalized_tokens, binding_hook_filter::BindingHookFilter,
    binding_hook_load_output::BindingHookLoadOutput,
    binding_hook_render_chunk_output::BindingHookRenderChunkOutput,
    binding_hook_resolve_id_extra_args::BindingHookResolveIdExtraArgs,
    binding_hook_resolve_id_output::BindingHookResolveIdOutput,
    binding_hook_transform_output::BindingHookTransformOutput,
    binding_plugin_transform_extra_args::BindingTransformHookExtraArgs,
    binding_render_chunk_meta_chunks::BindingRenderedChunkMeta,
  },
};

/// none is parallel js plugin
pub type BindingPluginOrParallelJsPluginPlaceholder<'env> =
  Option<Either<BindingPluginOptions, BindingBuiltinPlugin<'env>>>;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingPluginOptions {
  pub name: String,
  pub hook_usage: u32,
  #[napi(
    ts_type = "(ctx: BindingPluginContext, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable>"
  )]
  pub build_start:
    Option<MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, BindingNormalizedOptions)>>>,
  pub build_start_meta: Option<BindingPluginHookMeta>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraArgs) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>"
  )]
  pub resolve_id: Option<
    MaybeAsyncJsCallback<
      FnArgs<(BindingPluginContext, String, Option<String>, BindingHookResolveIdExtraArgs)>,
      Option<BindingHookResolveIdOutput>,
    >,
  >,
  pub resolve_id_meta: Option<BindingPluginHookMeta>,
  pub resolve_id_filter: Option<BindingHookFilter>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, specifier: string, importer: Nullable<string>) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>"
  )]
  pub resolve_dynamic_import: Option<
    MaybeAsyncJsCallback<
      FnArgs<(BindingPluginContext, String, Option<String>)>,
      Option<BindingHookResolveIdOutput>,
    >,
  >,
  pub resolve_dynamic_import_meta: Option<BindingPluginHookMeta>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>"
  )]
  pub load: Option<
    MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, String)>, Option<BindingHookLoadOutput>>,
  >,
  pub load_meta: Option<BindingPluginHookMeta>,
  pub load_filter: Option<BindingHookFilter>,

  #[napi(
    ts_type = "(ctx:  BindingTransformPluginContext, id: string, code: string, module_type: BindingTransformHookExtraArgs) => MaybePromise<VoidNullable<BindingHookTransformOutput>>"
  )]
  pub transform: Option<
    MaybeAsyncJsCallback<
      FnArgs<(BindingTransformPluginContext, String, String, BindingTransformHookExtraArgs)>,
      Option<BindingHookTransformOutput>,
    >,
  >,
  pub transform_meta: Option<BindingPluginHookMeta>,
  pub transform_filter: Option<BindingHookFilter>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>"
  )]
  pub module_parsed:
    Option<MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, BindingModuleInfo)>>>,
  pub module_parsed_meta: Option<BindingPluginHookMeta>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, error?: (Error | BindingError)[]) => MaybePromise<VoidNullable>"
  )]
  pub build_end: Option<
    MaybeAsyncJsCallback<
      FnArgs<(BindingPluginContext, Option<Vec<napi::Either<napi::JsError, BindingError>>>)>,
    >,
  >,
  pub build_end_meta: Option<BindingPluginHookMeta>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, code: string, chunk: BindingRenderedChunk, opts: BindingNormalizedOptions, meta: BindingRenderedChunkMeta) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>"
  )]
  pub render_chunk: Option<
    MaybeAsyncJsCallback<
      FnArgs<(
        BindingPluginContext,
        String,
        BindingRenderedChunk,
        BindingNormalizedOptions,
        BindingRenderedChunkMeta,
      )>,
      Option<BindingHookRenderChunkOutput>,
    >,
  >,
  pub render_chunk_meta: Option<BindingPluginHookMeta>,
  pub render_chunk_filter: Option<BindingHookFilter>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, chunk: BindingRenderedChunk) => MaybePromise<void | string>"
  )]
  pub augment_chunk_hash: Option<
    MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, BindingRenderedChunk)>, Option<String>>,
  >,
  pub augment_chunk_hash_meta: Option<BindingPluginHookMeta>,

  #[napi(ts_type = "(ctx: BindingPluginContext, opts: BindingNormalizedOptions) => void")]
  pub render_start:
    Option<MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, BindingNormalizedOptions)>>>,
  pub render_start_meta: Option<BindingPluginHookMeta>,

  #[napi(ts_type = "(ctx: BindingPluginContext, error: (Error | BindingError)[]) => void")]
  pub render_error: Option<
    MaybeAsyncJsCallback<
      FnArgs<(BindingPluginContext, Vec<napi::Either<napi::JsError, BindingError>>)>,
    >,
  >,
  pub render_error_meta: Option<BindingPluginHookMeta>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, bundle: BindingOutputs, isWrite: boolean, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable<JsChangedOutputs>>"
  )]
  pub generate_bundle: Option<
    MaybeAsyncJsCallback<
      FnArgs<(BindingPluginContext, BindingOutputs, bool, BindingNormalizedOptions)>,
      JsChangedOutputs,
    >,
  >,
  pub generate_bundle_meta: Option<BindingPluginHookMeta>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, bundle: BindingOutputs, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable<JsChangedOutputs>>"
  )]
  pub write_bundle: Option<
    MaybeAsyncJsCallback<
      FnArgs<(BindingPluginContext, BindingOutputs, BindingNormalizedOptions)>,
      JsChangedOutputs,
    >,
  >,
  pub write_bundle_meta: Option<BindingPluginHookMeta>,

  #[napi(ts_type = "(ctx: BindingPluginContext) => MaybePromise<VoidNullable>")]
  pub close_bundle: Option<MaybeAsyncJsCallback<FnArgs<(BindingPluginContext,)>>>,
  pub close_bundle_meta: Option<BindingPluginHookMeta>,

  #[napi(
    ts_type = "(ctx: BindingPluginContext, path: string, event: string) => MaybePromise<VoidNullable>"
  )]
  pub watch_change: Option<MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, String, String)>>>,
  pub watch_change_meta: Option<BindingPluginHookMeta>,

  #[napi(ts_type = "(ctx: BindingPluginContext) => MaybePromise<VoidNullable>")]
  pub close_watcher: Option<MaybeAsyncJsCallback<FnArgs<(BindingPluginContext,)>>>,
  pub close_watcher_meta: Option<BindingPluginHookMeta>,

  #[napi(ts_type = "(ctx: BindingPluginContext, chunk: BindingRenderedChunk) => void")]
  pub banner: Option<
    MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, BindingRenderedChunk)>, Option<String>>,
  >,
  pub banner_meta: Option<BindingPluginHookMeta>,

  #[napi(ts_type = "(ctx: BindingPluginContext, chunk: BindingRenderedChunk) => void")]
  pub footer: Option<
    MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, BindingRenderedChunk)>, Option<String>>,
  >,
  pub footer_meta: Option<BindingPluginHookMeta>,

  #[napi(ts_type = "(ctx: BindingPluginContext, chunk: BindingRenderedChunk) => void")]
  pub intro: Option<
    MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, BindingRenderedChunk)>, Option<String>>,
  >,
  pub intro_meta: Option<BindingPluginHookMeta>,

  #[napi(ts_type = "(ctx: BindingPluginContext, chunk: BindingRenderedChunk) => void")]
  pub outro: Option<
    MaybeAsyncJsCallback<FnArgs<(BindingPluginContext, BindingRenderedChunk)>, Option<String>>,
  >,
  pub outro_meta: Option<BindingPluginHookMeta>,
}

impl Debug for BindingPluginOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingPluginOptions").field("name", &self.name).finish_non_exhaustive()
  }
}

#[derive(Default, Debug)]
pub struct FilterExprCache {
  pub resolve_id: Option<Vec<FilterExprKind>>,
  pub load: Option<Vec<FilterExprKind>>,
  pub transform: Option<Vec<FilterExprKind>>,
  pub render_chunk: Option<Vec<FilterExprKind>>,
}
impl BindingPluginOptions {
  pub fn pre_compile_filter_expr(&self) -> FilterExprCache {
    let mut cache = FilterExprCache::default();
    if let Some(tokenss) = self.resolve_id_filter.as_ref().and_then(|item| item.value.as_ref()) {
      let filter_kind = tokenss
        .clone()
        .into_iter()
        .map(|tokens| filter_expression::parse(normalized_tokens(tokens)))
        .collect_vec();
      cache.resolve_id = Some(filter_kind);
    }

    if let Some(filter) = self.load_filter.as_ref().and_then(|item| item.value.as_ref()) {
      let filter_kind = filter
        .clone()
        .into_iter()
        .map(|tokens| filter_expression::parse(normalized_tokens(tokens)))
        .collect_vec();
      cache.load = Some(filter_kind);
    }

    if let Some(filter) = self.transform_filter.as_ref().and_then(|item| item.value.as_ref()) {
      let filter_kind = filter
        .clone()
        .into_iter()
        .map(|tokens| filter_expression::parse(normalized_tokens(tokens)))
        .collect_vec();
      cache.transform = Some(filter_kind);
    }

    if let Some(filter) = self.render_chunk_filter.as_ref().and_then(|item| item.value.as_ref()) {
      let filter_kind = filter
        .clone()
        .into_iter()
        .map(|tokens| filter_expression::parse(normalized_tokens(tokens)))
        .collect_vec();
      cache.render_chunk = Some(filter_kind);
    }

    cache
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingPluginWithIndex {
  pub index: u32,
  pub plugin: BindingPluginOptions,
}
