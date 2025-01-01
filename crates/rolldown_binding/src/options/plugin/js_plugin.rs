use crate::types::{
  binding_module_info::BindingModuleInfo,
  binding_normalized_options::BindingNormalizedOptions,
  binding_outputs::{to_js_diagnostic, update_outputs},
  js_callback::MaybeAsyncJsCallbackExt,
};
use anyhow::Ok;
use rolldown::ModuleType;
use rolldown_plugin::{Plugin, __inner::SharedPluginable, typedmap::TypedMapKey};
use rolldown_utils::pattern_filter::{self, FilterResult};
use std::{
  borrow::Cow,
  ops::Deref,
  path::{Path, PathBuf},
  sync::Arc,
};
use sugar_path::SugarPath;

use super::{
  binding_transform_context::BindingTransformPluginContext,
  types::{
    binding_hook_filter::BindingTransformHookFilter,
    binding_hook_resolve_id_extra_args::BindingHookResolveIdExtraArgs,
    binding_plugin_transform_extra_args::BindingTransformHookExtraArgs,
  },
  BindingPluginOptions,
};

#[derive(Hash, Debug, PartialEq, Eq)]
pub(crate) struct JsPluginContextResolveCustomArgId;

impl TypedMapKey for JsPluginContextResolveCustomArgId {
  type Value = u32;
}

#[derive(Debug)]
pub struct JsPlugin {
  pub(crate) inner: BindingPluginOptions,
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
    Self { inner }
  }

  pub(crate) fn new_shared(inner: BindingPluginOptions) -> SharedPluginable {
    Arc::new(Self { inner })
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
      cb.await_call((ctx.clone().into(), BindingNormalizedOptions::new(Arc::clone(args.options))))
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

    if let Some(resolve_id_filter) = &self.inner.resolve_id_filter {
      let stabilized_path = Path::new(args.specifier).relative(ctx.cwd());
      let normalized_id = stabilized_path.to_string_lossy();

      let matched = pattern_filter::filter(
        resolve_id_filter.exclude.as_deref(),
        resolve_id_filter.include.as_deref(),
        args.specifier,
        &normalized_id,
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
      cb.await_call((
        ctx.clone().into(),
        args.specifier.to_string(),
        args.importer.map(str::to_string),
        extra_args,
      ))
      .await?
      .map(Into::into),
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
    if let Some(cb) = &self.resolve_dynamic_import {
      Ok(
        cb.await_call((
          ctx.clone().into(),
          args.specifier.to_string(),
          args.importer.map(str::to_string),
        ))
        .await?
        .map(Into::into),
      )
    } else {
      Ok(None)
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

    if let Some(load_filter) = &self.load_filter {
      let stabilized_path = Path::new(args.id).relative(ctx.cwd());
      let normalized_id = stabilized_path.to_string_lossy();

      let matched = pattern_filter::filter(
        load_filter.exclude.as_deref(),
        load_filter.include.as_deref(),
        args.id,
        &normalized_id,
      )
      .inner();

      if !matched {
        return Ok(None);
      }
    }

    cb.await_call((ctx.clone().into(), args.id.to_string()))
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

    if !filter_transform(
      self.transform_filter.as_ref(),
      args.id,
      ctx.inner.cwd(),
      args.module_type,
      args.code,
    )? {
      return Ok(None);
    }

    let extra_args = BindingTransformHookExtraArgs { module_type: args.module_type.to_string() };

    cb.await_call((
      BindingTransformPluginContext::new(Arc::clone(&ctx)),
      args.code.to_string(),
      args.id.to_string(),
      extra_args,
    ))
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
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.module_parsed {
      cb.await_call((ctx.clone().into(), BindingModuleInfo::new(module_info))).await?;
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
      cb.await_call((
        ctx.clone().into(),
        args.map(|args| {
          args
            .errors
            .iter()
            .map(|diagnostic| to_js_diagnostic(diagnostic, args.cwd.clone()))
            .collect()
        }),
      ))
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
      cb.await_call((ctx.clone().into(), BindingNormalizedOptions::new(Arc::clone(args.options))))
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
    args: &rolldown_plugin::HookAddonArgs<'_>,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if let Some(cb) = &self.banner {
      Ok(
        cb.await_call((ctx.clone().into(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  fn banner_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.banner_meta.as_ref().map(Into::into)
  }

  async fn intro(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs<'_>,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if let Some(cb) = &self.intro {
      Ok(
        cb.await_call((ctx.clone().into(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  fn intro_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.intro_meta.as_ref().map(Into::into)
  }

  async fn outro(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs<'_>,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if let Some(cb) = &self.outro {
      Ok(
        cb.await_call((ctx.clone().into(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  fn outro_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.outro_meta.as_ref().map(Into::into)
  }

  async fn footer(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs<'_>,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if let Some(cb) = &self.footer {
      Ok(
        cb.await_call((ctx.clone().into(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
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
    if let Some(cb) = &self.render_chunk {
      Ok(
        cb.await_call((
          ctx.clone().into(),
          args.code.to_string(),
          args.chunk.clone().into(),
          BindingNormalizedOptions::new(Arc::clone(args.options)),
        ))
        .await?
        .map(TryInto::try_into)
        .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  fn render_chunk_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.render_chunk_meta.as_ref().map(Into::into)
  }

  async fn augment_chunk_hash(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    chunk: &rolldown_common::RollupRenderedChunk,
  ) -> rolldown_plugin::HookAugmentChunkHashReturn {
    if let Some(cb) = &self.augment_chunk_hash {
      Ok(cb.await_call((ctx.clone().into(), chunk.clone().into())).await?)
    } else {
      Ok(None)
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
      cb.await_call((
        ctx.clone().into(),
        args
          .errors
          .iter()
          .map(|diagnostic| to_js_diagnostic(diagnostic, args.cwd.clone()))
          .collect(),
      ))
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
        .await_call((
          ctx.clone().into(),
          args.bundle.clone().into(),
          args.is_write,
          BindingNormalizedOptions::new(Arc::clone(args.options)),
        ))
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
        .await_call((
          ctx.clone().into(),
          args.bundle.clone().into(),
          BindingNormalizedOptions::new(Arc::clone(args.options)),
        ))
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
      cb.await_call(ctx.clone().into()).await?;
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
      cb.await_call((ctx.clone().into(), path.to_string(), event.to_string())).await?;
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
      cb.await_call(ctx.clone().into()).await?;
    }
    Ok(())
  }

  fn close_watcher_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.close_watcher_meta.as_ref().map(Into::into)
  }
}

/// If the transform hook is filtered out and need to be skipped.
/// Using `Option<bool>` for better programming experience.
/// return `None` means it is early return, should not be skipped.
/// return `Some(false)` means it should be skipped.
/// return `Some(true)` means it should not be skipped.
/// Since transform has three different filter, so we need to check all of them.
fn filter_transform(
  transform_filter: Option<&BindingTransformHookFilter>,
  id: &str,
  cwd: &PathBuf,
  module_type: &ModuleType,
  code: &str,
) -> anyhow::Result<bool> {
  let Some(transform_filter) = transform_filter else {
    return Ok(true);
  };

  let mut fallback_ret = if let Some(ref module_type_filter) = transform_filter.module_type {
    if module_type_filter.iter().any(|ty| ty.as_ref() == module_type) {
      return Ok(true);
    }
    false
  } else {
    true
  };

  if let Some(ref id_filter) = transform_filter.id {
    let stabilized_path = Path::new(id).relative(cwd);
    let normalized_id = stabilized_path.to_string_lossy();

    let id_res = pattern_filter::filter(
      id_filter.exclude.as_deref(),
      id_filter.include.as_deref(),
      id,
      &normalized_id,
    );

    // it matched by `exclude` or `include`, early return
    if let FilterResult::Match(id_res) = id_res {
      return Ok(id_res);
    }

    fallback_ret = fallback_ret && id_res.inner();
  }

  if let Some(ref code_filter) = transform_filter.code {
    let code_res = pattern_filter::filter_code(
      code_filter.exclude.as_deref(),
      code_filter.include.as_deref(),
      code,
    );

    // it matched by `exclude` or `include`, early return
    if let FilterResult::Match(code_res) = code_res {
      return Ok(code_res);
    }

    fallback_ret = fallback_ret && code_res.inner();
  }

  Ok(fallback_ret)
}
