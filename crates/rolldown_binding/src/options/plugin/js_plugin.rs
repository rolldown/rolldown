use crate::types::{
  binding_module_info::BindingModuleInfo,
  binding_normalized_options::BindingNormalizedOptions,
  binding_outputs::{to_js_diagnostic, update_outputs},
  binding_rendered_chunk::BindingRenderedChunk,
  js_callback::MaybeAsyncJsCallbackExt,
};
use napi::bindgen_prelude::FnArgs;
use rolldown_common::NormalModule;
use rolldown_plugin::{__inner::SharedPluginable, HookUsage, Plugin, typedmap::TypedMapKey};
use rolldown_utils::{
  filter_expression::filter_exprs_interpreter,
  pattern_filter::{self},
};
use std::{borrow::Cow, ops::Deref, sync::Arc};
use tracing::{Instrument, debug_span};

use super::{
  BindingPluginOptions, FilterExprCache,
  binding_transform_context::BindingTransformPluginContext,
  js_plugin_filter::{filter_render_chunk, filter_transform},
  types::{
    binding_hook_resolve_id_extra_args::BindingHookResolveIdExtraArgs,
    binding_plugin_transform_extra_args::BindingTransformHookExtraArgs,
    binding_render_chunk_meta_chunks::BindingRenderedChunkMeta,
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

impl Deref for JsPlugin {
  type Target = BindingPluginOptions;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl JsPlugin {
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub(super) fn new(inner: BindingPluginOptions) -> Self {
    let filter_expr_cache = inner.pre_compile_filter_expr();
    Self { inner, filter_expr_cache }
  }

  pub(crate) fn new_shared(inner: BindingPluginOptions) -> SharedPluginable {
    let filter_expr_cache = inner.pre_compile_filter_expr();
    Arc::new(Self { inner, filter_expr_cache })
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
      .await?;
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
        ctx.cwd().to_string_lossy().as_ref(),
      ) {
        return Ok(None);
      }
    } else if let Some(resolve_id_filter) = &self.inner.resolve_id_filter {
      let matched = pattern_filter::filter(
        resolve_id_filter.exclude.as_deref(),
        resolve_id_filter.include.as_deref(),
        args.specifier,
        ctx.cwd().to_string_lossy().as_ref(),
      )
      .inner();

      if !matched {
        return Ok(None);
      }
    }

    let extra_args = BindingHookResolveIdExtraArgs {
      is_entry: args.is_entry,
      kind: args.kind.to_string(),
      custom: args
        .custom
        .get::<JsPluginContextResolveCustomArgId>(&JsPluginContextResolveCustomArgId)
        .map(|v| *v),
    };

    Ok(
      cb.await_call(
        (
          ctx.clone().into(),
          args.specifier.to_string(),
          args.importer.map(str::to_string),
          extra_args,
        )
          .into(),
      )
      .instrument(debug_span!("resolve_id_hook", plugin_name = self.name))
      .await?
      .map(TryInto::try_into)
      .transpose()?,
    )
  }

  fn resolve_id_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.resolve_id_meta.as_ref().map(Into::into)
  }

  async fn resolve_dynamic_import(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    match &self.resolve_dynamic_import {
      Some(cb) => Ok(
        cb.await_call(
          (ctx.clone().into(), args.specifier.to_string(), args.importer.map(str::to_string))
            .into(),
        )
        .instrument(debug_span!("resolve_dynamic_import_hook", plugin_name = self.name))
        .await?
        .map(TryInto::try_into)
        .transpose()?,
      ),
      _ => Ok(None),
    }
  }

  fn resolve_dynamic_import_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.resolve_dynamic_import_meta.as_ref().map(Into::into)
  }

  async fn load(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    let Some(cb) = &self.load else { return Ok(None) };

    if let Some(ref v) = self.filter_expr_cache.load {
      if !filter_exprs_interpreter(
        v,
        Some(args.id),
        None,
        None,
        ctx.cwd().to_string_lossy().as_ref(),
      ) {
        return Ok(None);
      }
    } else if let Some(load_filter) = &self.load_filter {
      let matched = pattern_filter::filter(
        load_filter.exclude.as_deref(),
        load_filter.include.as_deref(),
        args.id,
        ctx.cwd().to_string_lossy().as_ref(),
      )
      .inner();

      if !matched {
        return Ok(None);
      }
    }

    cb.await_call((ctx.clone().into(), args.id.to_string()).into())
      .instrument(debug_span!("load_hook", plugin_name = self.name))
      .await?
      .map(TryInto::try_into)
      .transpose()
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
        Some(args.code),
        Some(args.module_type.to_string().as_ref()),
        ctx.inner.cwd().to_string_lossy().as_ref(),
      ) {
        return Ok(None);
      }
    } else if !filter_transform(
      self.transform_filter.as_ref(),
      args.id,
      ctx.inner.cwd(),
      args.module_type,
      args.code,
    ) {
      return Ok(None);
    }

    let extra_args = BindingTransformHookExtraArgs { module_type: args.module_type.to_string() };

    cb.await_call(
      (
        BindingTransformPluginContext::new(Arc::clone(&ctx)),
        args.code.to_string(),
        args.id.to_string(),
        extra_args,
      )
        .into(),
    )
    .instrument(debug_span!("transform_hook", plugin_name = self.name))
    .await?
    .map(TryInto::try_into)
    .transpose()
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
      cb.await_call((ctx.clone().into(), BindingModuleInfo::new(module_info)).into())
        .instrument(debug_span!("module_parsed_hook", plugin_name = self.name))
        .await?;
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
              .map(|diagnostic| to_js_diagnostic(diagnostic, args.cwd.clone()))
              .collect()
          }),
        )
          .into(),
      )
      .instrument(debug_span!("build_end_hook", plugin_name = self.name))
      .await?;
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
      .await?;
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
        .transpose()?,
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
        .transpose()?,
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
        .transpose()?,
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
        .transpose()?,
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
        Some(&args.code),
        None,
        ctx.cwd().to_string_lossy().as_ref(),
      ) {
        return Ok(None);
      }
    } else if !filter_render_chunk(&args.code, self.render_chunk_filter.as_ref()) {
      return Ok(None);
    }

    cb.await_call(
      (
        ctx.clone().into(),
        args.code.to_string(),
        BindingRenderedChunk::new(Arc::clone(&args.chunk)),
        BindingNormalizedOptions::new(Arc::clone(args.options)),
        BindingRenderedChunkMeta::new(Arc::clone(&args.chunks)),
      )
        .into(),
    )
    .instrument(debug_span!("render_chunk_hook", plugin_name = self.name))
    .await?
    .map(TryInto::try_into)
    .transpose()
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
        cb.await_call((ctx.clone().into(), BindingRenderedChunk::new(chunk)).into())
          .instrument(debug_span!("augment_chunk_hash_hook", plugin_name = self.name))
          .await?,
      ),
      _ => Ok(None),
    }
  }

  fn augment_chunk_hash_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.augment_chunk_hash_meta.as_ref().map(Into::into)
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
            .map(|diagnostic| to_js_diagnostic(diagnostic, args.cwd.clone()))
            .collect(),
        )
          .into(),
      )
      .instrument(debug_span!("render_error_hook", plugin_name = self.name))
      .await?;
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
      let changed = cb
        .await_call(
          (
            ctx.clone().into(),
            args.bundle.clone().into(),
            args.is_write,
            BindingNormalizedOptions::new(Arc::clone(args.options)),
          )
            .into(),
        )
        .instrument(debug_span!("generate_bundle_hook", plugin_name = self.name))
        .await?;
      update_outputs(args.bundle, changed)?;
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
      let changed = cb
        .await_call(
          (
            ctx.clone().into(),
            args.bundle.clone().into(),
            BindingNormalizedOptions::new(Arc::clone(args.options)),
          )
            .into(),
        )
        .instrument(debug_span!("write_bundle_hook", plugin_name = self.name))
        .await?;
      update_outputs(args.bundle, changed)?;
    }
    Ok(())
  }

  fn write_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.write_bundle_meta.as_ref().map(Into::into)
  }

  async fn close_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.close_bundle {
      cb.await_call(FnArgs { data: (ctx.clone().into(),) })
        .instrument(debug_span!("close_bundle_hook", plugin_name = self.name))
        .await?;
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
        .await?;
    }
    Ok(())
  }

  fn watch_change_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.watch_change_meta.as_ref().map(Into::into)
  }

  async fn close_watcher(
    &self,
    ctx: &rolldown_plugin::PluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.close_watcher {
      cb.await_call(FnArgs { data: (ctx.clone().into(),) })
        .instrument(debug_span!("close_watcher_hook", plugin_name = self.name))
        .await?;
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
