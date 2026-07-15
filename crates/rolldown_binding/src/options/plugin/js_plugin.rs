use crate::types::{
  binding_module_info::BindingModuleInfo,
  binding_normalized_options::BindingNormalizedOptions,
  binding_outputs::{JsChangedOutputs, to_binding_error},
  binding_rendered_chunk::BindingRenderedChunk,
  js_callback::MaybeAsyncJsCallbackExt,
};
use anyhow::Context;
use napi::bindgen_prelude::FnArgs;
use rolldown_common::NormalModule;
use rolldown_plugin::{__inner::SharedPluginable, HookUsage, Plugin, typedmap::TypedMapKey};
use rolldown_utils::filter_expression::filter_exprs_interpreter;
use std::{borrow::Cow, ops::Deref, sync::Arc};
use tracing::{Instrument, debug_span};

use super::{
  BindingPluginOptions, FilterExprCache,
  binding_load_context::BindingLoadPluginContext,
  binding_transform_context::BindingTransformPluginContext,
  types::{
    binding_hook_resolve_file_url_args::BindingHookResolveFileUrlArgs,
    binding_hook_resolve_id_extra_args::BindingHookResolveIdExtraArgs,
    binding_hot_update_args::BindingHotUpdateArgs,
    binding_plugin_transform_extra_args::BindingTransformHookExtraArgs,
    binding_render_chunk_meta_chunks::BindingRenderedChunkMeta,
    binding_shared_string::BindingSharedString,
  },
};

#[derive(Hash, Debug, PartialEq, Eq)]
pub struct JsPluginContextResolveCustomArgId;

impl TypedMapKey for JsPluginContextResolveCustomArgId {
  type Value = u32;
}
#[derive(Debug)]
pub struct JsPlugin {
  pub(crate) inner: BindingPluginOptions,
  /// Since there at most three key in the cache, use vec should always faster than hashmap
  pub(crate) filter_expr_cache: FilterExprCache,
}

/// Records whether a per-module hook (resolveId / load / transform) produced a
/// value or returned nothing, onto the hook's tracing span's `result_kind`
/// field. In a `chrome-json` trace the value lands on the span's END event, so
/// counting `result_kind == "null"` per plugin gives an accurate early-return
/// tally (the primary "this hook should have a `filter`" signal) without any
/// JS-side instrumentation. `Err` results are left unrecorded — a thrown hook
/// is not an early return. The span must declare `result_kind = field::Empty`.
#[inline]
fn record_hook_result<T>(span: &tracing::Span, result: &anyhow::Result<Option<T>>) {
  if let Ok(value) = result {
    span.record("result_kind", if value.is_some() { "value" } else { "null" });
  }
}

impl Deref for JsPlugin {
  type Target = BindingPluginOptions;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl JsPlugin {
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub(super) fn new(inner: BindingPluginOptions) -> napi::Result<Self> {
    let filter_expr_cache = inner.pre_compile_filter_expr()?;
    Ok(Self { inner, filter_expr_cache })
  }

  pub(crate) fn new_shared(inner: BindingPluginOptions) -> napi::Result<SharedPluginable> {
    let filter_expr_cache = inner.pre_compile_filter_expr()?;
    Ok(Arc::new(Self { inner, filter_expr_cache }))
  }
}

impl Plugin for JsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Owned(self.name.clone())
  }

  // --- Build hooks ---

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_start {
      cb.await_call(
        (ctx.clone().into(), BindingNormalizedOptions::new(Arc::clone(args.options))).into(),
      )
      .instrument(debug_span!("build_start_hook", plugin_name = self.name))
      .await
      .context("buildStart hook threw an error")?;
    }
    Ok(())
  }

  fn build_start_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.build_start_meta.as_ref().map(Into::into)
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    let Some(cb) = &self.resolve_id else { return Ok(None) };
    if let Some(ref v) = self.filter_expr_cache.resolve_id {
      if !filter_exprs_interpreter(
        v,
        Some(args.specifier),
        None,
        None,
        args.importer,
        ctx.cwd().to_string_lossy().as_ref(),
      ) {
        return Ok(None);
      }
    }

    let extra_args = BindingHookResolveIdExtraArgs {
      is_entry: args.is_entry,
      kind: args.kind.to_string(),
      custom: args
        .custom
        .get::<JsPluginContextResolveCustomArgId>(&JsPluginContextResolveCustomArgId)
        .copied(),
    };

    let span =
      debug_span!("resolve_id_hook", plugin_name = self.name, result_kind = tracing::field::Empty);
    let result = cb
      .await_call(
        (
          ctx.clone().into(),
          args.specifier.to_string(),
          args.importer.map(str::to_string),
          extra_args,
        )
          .into(),
      )
      .instrument(span.clone())
      .await?
      .map(TryInto::try_into)
      .transpose()
      .with_context(|| {
        format!(
          "resolveId hook threw an error for specifier={} importer={}",
          args.specifier,
          args.importer.unwrap_or("undefined")
        )
      });
    record_hook_result(&span, &result);
    result
  }

  fn resolve_id_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.resolve_id_meta.as_ref().map(Into::into)
  }

  async fn resolve_dynamic_import(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    let Some(cb) = &self.resolve_dynamic_import else { return Ok(None) };
    let span = debug_span!(
      "resolve_dynamic_import_hook",
      plugin_name = self.name,
      result_kind = tracing::field::Empty
    );
    let result = cb
      .await_call(
        (ctx.clone().into(), args.specifier.to_string(), args.importer.map(str::to_string)).into(),
      )
      .instrument(span.clone())
      .await?
      .map(TryInto::try_into)
      .transpose()
      .with_context(|| {
        format!(
          "resolveDynamicImport hook threw an error for specifier={} importer={}",
          args.specifier,
          args.importer.unwrap_or("undefined")
        )
      });
    record_hook_result(&span, &result);
    result
  }

  fn resolve_dynamic_import_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.resolve_dynamic_import_meta.as_ref().map(Into::into)
  }

  async fn load(
    &self,
    ctx: rolldown_plugin::SharedLoadPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    let Some(cb) = &self.load else { return Ok(None) };

    if let Some(ref v) = self.filter_expr_cache.load {
      if !filter_exprs_interpreter(
        v,
        Some(args.id),
        None,
        None,
        None,
        ctx.cwd().to_string_lossy().as_ref(),
      ) {
        return Ok(None);
      }
    }

    let binding_ctx = BindingLoadPluginContext::new(Arc::clone(&ctx));
    let span =
      debug_span!("load_hook", plugin_name = self.name, result_kind = tracing::field::Empty);
    let result = cb
      .await_call((binding_ctx, args.id.to_string()).into())
      .instrument(span.clone())
      .await?
      .map(TryInto::try_into)
      .transpose()
      .with_context(|| format!("load hook threw an error for id={}", args.id));
    record_hook_result(&span, &result);
    result
  }

  fn load_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.load_meta.as_ref().map(Into::into)
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let Some(cb) = &self.transform else { return Ok(None) };
    // Custom field have higher priority, it will override the default filter
    if let Some(ref v) = self.filter_expr_cache.transform {
      if !filter_exprs_interpreter(
        v,
        Some(args.id),
        Some(args.code.as_str()),
        Some(args.module_type.to_string().as_ref()),
        None,
        ctx.cwd().to_string_lossy().as_ref(),
      ) {
        return Ok(None);
      }
    }

    let extra_args = BindingTransformHookExtraArgs { module_type: args.module_type.to_string() };

    let span =
      debug_span!("transform_hook", plugin_name = self.name, result_kind = tracing::field::Empty);
    let result = cb
      .await_call(
        (
          BindingTransformPluginContext::new(Arc::clone(&ctx)),
          BindingSharedString::from(args.code.clone()),
          args.id.to_string(),
          extra_args,
        )
          .into(),
      )
      .instrument(span.clone())
      .await?
      .map(TryInto::try_into)
      .transpose()
      .with_context(|| format!("transform hook threw an error for id={}", args.id));
    record_hook_result(&span, &result);
    result
  }

  fn transform_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.transform_meta.as_ref().map(Into::into)
  }

  async fn module_parsed(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    module_info: Arc<rolldown_common::ModuleInfo>,
    _normal_module: &NormalModule,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.module_parsed {
      cb.await_call((ctx.clone().into(), BindingModuleInfo::new(Arc::clone(&module_info))).into())
        .instrument(debug_span!("module_parsed_hook", plugin_name = self.name))
        .await
        .with_context(|| {
          format!("moduleParsed hook threw an error for id={}", module_info.id.as_ref())
        })?;
    }
    Ok(())
  }

  fn module_parsed_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.module_parsed_meta.as_ref().map(Into::into)
  }

  async fn build_end(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_end {
      cb.await_call(
        (
          ctx.clone().into(),
          args.map(|args| {
            args
              .errors
              .iter()
              .map(|diagnostic| to_binding_error(diagnostic, args.cwd.clone()))
              .collect()
          }),
        )
          .into(),
      )
      .instrument(debug_span!("build_end_hook", plugin_name = self.name))
      .await
      .context("buildEnd hook threw an error")?;
    }
    Ok(())
  }

  fn build_end_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.build_end_meta.as_ref().map(Into::into)
  }

  // --- Generate hooks ---

  async fn render_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.render_start {
      cb.await_call(
        (ctx.clone().into(), BindingNormalizedOptions::new(Arc::clone(args.options))).into(),
      )
      .instrument(debug_span!("render_start_hook", plugin_name = self.name))
      .await
      .context("renderStart hook threw an error")?;
    }
    Ok(())
  }

  fn render_start_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.render_start_meta.as_ref().map(Into::into)
  }

  async fn banner(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    match &self.banner {
      Some(cb) => Ok(
        cb.await_call(
          (ctx.clone().into(), BindingRenderedChunk::new(Arc::clone(&args.chunk))).into(),
        )
        .instrument(debug_span!("banner_hook", plugin_name = self.name))
        .await?
        .map(TryInto::try_into)
        .transpose()
        .with_context(|| format!("banner hook threw an error for chunkName={}", args.chunk.name))?,
      ),
      _ => Ok(None),
    }
  }

  fn banner_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.banner_meta.as_ref().map(Into::into)
  }

  async fn intro(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    match &self.intro {
      Some(cb) => Ok(
        cb.await_call(
          (ctx.clone().into(), BindingRenderedChunk::new(Arc::clone(&args.chunk))).into(),
        )
        .instrument(debug_span!("intro_hook", plugin_name = self.name))
        .await?
        .map(TryInto::try_into)
        .transpose()
        .with_context(|| format!("intro hook threw an error for chunkName={}", args.chunk.name))?,
      ),
      _ => Ok(None),
    }
  }

  fn intro_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.intro_meta.as_ref().map(Into::into)
  }

  async fn outro(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    match &self.outro {
      Some(cb) => Ok(
        cb.await_call(
          (ctx.clone().into(), BindingRenderedChunk::new(Arc::clone(&args.chunk))).into(),
        )
        .instrument(debug_span!("outro_hook", plugin_name = self.name))
        .await?
        .map(TryInto::try_into)
        .transpose()
        .with_context(|| format!("outro hook threw an error for chunkName={}", args.chunk.name))?,
      ),
      _ => Ok(None),
    }
  }

  fn outro_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.outro_meta.as_ref().map(Into::into)
  }

  async fn footer(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    match &self.footer {
      Some(cb) => Ok(
        cb.await_call(
          (ctx.clone().into(), BindingRenderedChunk::new(Arc::clone(&args.chunk))).into(),
        )
        .instrument(debug_span!("footer_hook", plugin_name = self.name))
        .await?
        .map(TryInto::try_into)
        .transpose()
        .with_context(|| format!("footer hook threw an error for chunkName={}", args.chunk.name))?,
      ),
      _ => Ok(None),
    }
  }

  fn footer_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.footer_meta.as_ref().map(Into::into)
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let Some(cb) = &self.render_chunk else { return Ok(None) };

    if let Some(ref v) = self.filter_expr_cache.render_chunk {
      if !filter_exprs_interpreter(
        v,
        None,
        Some(args.code.as_str()),
        None,
        None,
        ctx.cwd().to_string_lossy().as_ref(),
      ) {
        return Ok(None);
      }
    }

    let span = debug_span!(
      "render_chunk_hook",
      plugin_name = self.name,
      result_kind = tracing::field::Empty
    );
    let result = cb
      .await_call(
        (
          ctx.clone().into(),
          BindingSharedString::from(Arc::clone(&args.code)),
          BindingRenderedChunk::new(Arc::clone(&args.chunk)),
          BindingNormalizedOptions::new(Arc::clone(args.options)),
          BindingRenderedChunkMeta::new(Arc::clone(&args.chunks)),
        )
          .into(),
      )
      .instrument(span.clone())
      .await?
      .map(TryInto::try_into)
      .transpose()
      .with_context(|| {
        format!("renderChunk hook threw an error for chunkName={}", args.chunk.name)
      });
    record_hook_result(&span, &result);
    result
  }

  fn render_chunk_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.render_chunk_meta.as_ref().map(Into::into)
  }

  async fn augment_chunk_hash(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    chunk: Arc<rolldown_common::RollupRenderedChunk>,
  ) -> rolldown_plugin::HookAugmentChunkHashReturn {
    match &self.augment_chunk_hash {
      Some(cb) => Ok(
        cb.await_call((ctx.clone().into(), BindingRenderedChunk::new(Arc::clone(&chunk))).into())
          .instrument(debug_span!("augment_chunk_hash_hook", plugin_name = self.name))
          .await
          .with_context(|| {
            format!("augmentChunkHash hook threw an error for chunkName={}", chunk.name)
          })?,
      ),
      _ => Ok(None),
    }
  }

  fn augment_chunk_hash_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.augment_chunk_hash_meta.as_ref().map(Into::into)
  }

  async fn resolve_file_url(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveFileUrlArgs<'_>,
  ) -> rolldown_plugin::HookResolveFileUrlReturn {
    match &self.resolve_file_url {
      Some(cb) => Ok(
        cb.await_call(
          (
            ctx.clone().into(),
            BindingHookResolveFileUrlArgs {
              chunk_id: args.chunk_id.to_string(),
              file_name: args.file_name.to_string(),
              format: args.format.as_str().to_string(),
              module_id: args.module_id.to_string(),
              reference_id: args.reference_id.to_string(),
              relative_path: args.relative_path.to_string(),
            },
          )
            .into(),
        )
        .instrument(debug_span!("resolve_file_url_hook", plugin_name = self.name))
        .await
        .with_context(|| {
          format!("resolveFileUrl hook threw an error for referenceId={}", args.reference_id)
        })?,
      ),
      _ => Ok(None),
    }
  }

  fn resolve_file_url_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.resolve_file_url_meta.as_ref().map(Into::into)
  }

  async fn render_error(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderErrorArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.render_error {
      cb.await_call(
        (
          ctx.clone().into(),
          args
            .errors
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, args.cwd.clone()))
            .collect(),
        )
          .into(),
      )
      .instrument(debug_span!("render_error_hook", plugin_name = self.name))
      .await
      .context("renderError hook threw an error")?;
    }
    Ok(())
  }

  fn render_error_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.render_error_meta.as_ref().map(Into::into)
  }

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.generate_bundle {
      let mut changed: JsChangedOutputs = cb
        .await_call(
          (
            ctx.clone().into(),
            napi::Either::B(args.bundle.clone().into()),
            args.is_write,
            BindingNormalizedOptions::new(Arc::clone(args.options)),
          )
            .into(),
        )
        .instrument(debug_span!("generate_bundle_hook", plugin_name = self.name))
        .await
        .context("generateBundle hook threw an error")?;
      changed.apply_changes(args.bundle)?;
    }
    Ok(())
  }

  fn generate_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.generate_bundle_meta.as_ref().map(Into::into)
  }

  async fn write_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookWriteBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.write_bundle {
      let mut changed: JsChangedOutputs = cb
        .await_call(
          (
            ctx.clone().into(),
            napi::Either::B(args.bundle.clone().into()),
            BindingNormalizedOptions::new(Arc::clone(args.options)),
          )
            .into(),
        )
        .instrument(debug_span!("write_bundle_hook", plugin_name = self.name))
        .await
        .context("writeBundle hook threw an error")?;
      changed.apply_changes(args.bundle)?;
    }
    Ok(())
  }

  fn write_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.write_bundle_meta.as_ref().map(Into::into)
  }

  async fn close_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: Option<&rolldown_plugin::HookCloseBundleArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.close_bundle {
      cb.await_call(
        (
          ctx.clone().into(),
          args.map(|args| {
            args
              .errors
              .iter()
              .map(|diagnostic| to_binding_error(diagnostic, args.cwd.clone()))
              .collect()
          }),
        )
          .into(),
      )
      .instrument(debug_span!("close_bundle_hook", plugin_name = self.name))
      .await
      .context("closeBundle hook threw an error")?;
    }
    Ok(())
  }

  fn close_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.close_bundle_meta.as_ref().map(Into::into)
  }

  async fn watch_change(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    path: &str,
    event: rolldown_common::WatcherChangeKind,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.watch_change {
      cb.await_call((ctx.clone().into(), path.to_string(), event.to_string()).into())
        .instrument(debug_span!("watch_change_hook", plugin_name = self.name))
        .await
        .with_context(|| {
          format!("watchChange hook threw an error for path={path} event={event}")
        })?;
    }
    Ok(())
  }

  fn watch_change_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.watch_change_meta.as_ref().map(Into::into)
  }

  async fn hot_update(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookHotUpdateArgs,
  ) -> rolldown_plugin::HookHotUpdateReturn {
    let Some(cb) = &self.hot_update else { return Ok(None) };
    let binding_args = BindingHotUpdateArgs {
      kind: args.kind.to_string(),
      file: args.file.to_string(),
      modules: args.modules.iter().map(ToString::to_string).collect(),
    };
    let result = cb
      .await_call((ctx.clone().into(), binding_args).into())
      .instrument(debug_span!("hot_update_hook", plugin_name = self.name))
      .await
      .with_context(|| format!("hotUpdate hook threw an error for file={}", args.file))?;
    Ok(result.map(|modules| modules.into_iter().map(arcstr::ArcStr::from).collect()))
  }

  fn hot_update_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.hot_update_meta.as_ref().map(Into::into)
  }

  async fn close_watcher(
    &self,
    ctx: &rolldown_plugin::PluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.close_watcher {
      cb.await_call(FnArgs { data: (ctx.clone().into(),) })
        .instrument(debug_span!("close_watcher_hook", plugin_name = self.name))
        .await
        .context("closeWatcher hook threw an error")?;
    }
    Ok(())
  }

  fn close_watcher_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.close_watcher_meta.as_ref().map(Into::into)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::from_bits(self.inner.hook_usage).expect("Failed to register hook usage")
  }
}
