use crate::types::{
  binding_module_info::BindingModuleInfo, binding_outputs::BindingOutputs,
  js_callback::MaybeAsyncJsCallbackExt,
};
use rolldown_plugin::{
  Plugin, __inner::SharedPluginable, typedmap::TypedMapKey, LoadHookFilter, ResolvedIdHookFilter,
  TransformHookFilter,
};
use rolldown_utils::unique_arc::UniqueArc;
use std::{
  borrow::Cow,
  mem,
  ops::Deref,
  sync::{Arc, Mutex},
};

use super::{
  binding_transform_context::BindingTransformPluginContext,
  types::binding_hook_resolve_id_extra_args::BindingHookResolveIdExtraArgs,
  types::binding_plugin_transform_extra_args::BindingTransformHookExtraArgs, BindingPluginOptions,
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
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_start {
      cb.await_call(ctx.clone().into()).await?;
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
    if let Some(cb) = &self.resolve_id {
      let custom = args
        .custom
        .get::<JsPluginContextResolveCustomArgId>(&JsPluginContextResolveCustomArgId)
        .map(|v| *v);
      Ok(
        cb.await_call((
          ctx.clone().into(),
          args.specifier.to_string(),
          args.importer.map(str::to_string),
          BindingHookResolveIdExtraArgs {
            is_entry: args.is_entry,
            kind: args.kind.to_string(),
            custom,
          },
        ))
        .await?
        .map(Into::into),
      )
    } else {
      Ok(None)
    }
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
    if let Some(cb) = &self.load {
      Ok(
        cb.await_call((ctx.clone().into(), args.id.to_string()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  fn load_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.load_meta.as_ref().map(Into::into)
  }

  async fn transform(
    &self,
    ctx: &rolldown_plugin::TransformPluginContext<'_>,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if let Some(cb) = &self.transform {
      Ok(
        cb.await_call((
          BindingTransformPluginContext::new(unsafe { std::mem::transmute(ctx) }),
          args.code.to_string(),
          args.id.to_string(),
          BindingTransformHookExtraArgs { module_type: args.module_type.to_string() },
        ))
        .await?
        .map(TryInto::try_into)
        .transpose()?,
      )
    } else {
      Ok(None)
    }
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
    args: Option<&rolldown_plugin::HookBuildEndArgs>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_end {
      cb.await_call((ctx.clone().into(), args.map(|a| a.error.to_string()))).await?;
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
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.render_start {
      cb.await_call(ctx.clone().into()).await?;
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
        cb.await_call((ctx.clone().into(), args.code.to_string(), args.chunk.clone().into()))
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
    args: &rolldown_plugin::HookRenderErrorArgs,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.render_error {
      cb.await_call((ctx.clone().into(), args.error.to_string())).await?;
    }
    Ok(())
  }

  fn render_error_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.render_error_meta.as_ref().map(Into::into)
  }

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    bundle: &mut Vec<rolldown_common::Output>,
    is_write: bool,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.generate_bundle {
      let old_bundle = UniqueArc::new(Mutex::new(mem::take(bundle)));
      cb.await_call((ctx.clone().into(), BindingOutputs::new(old_bundle.weak_ref()), is_write))
        .await?;
      *bundle = old_bundle.into_inner().into_inner()?;
    }
    Ok(())
  }

  fn generate_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    self.generate_bundle_meta.as_ref().map(Into::into)
  }

  async fn write_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    bundle: &mut Vec<rolldown_common::Output>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.write_bundle {
      let old_bundle = UniqueArc::new(Mutex::new(mem::take(bundle)));
      cb.await_call((ctx.clone().into(), BindingOutputs::new(old_bundle.weak_ref()))).await?;
      *bundle = old_bundle.into_inner().into_inner()?;
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

  fn transform_filter(&self) -> anyhow::Result<Option<TransformHookFilter>> {
    match self.inner.transform_filter {
      Some(ref item) => {
        let filter = TransformHookFilter::try_from(item.clone())?;
        Ok(Some(filter))
      }
      None => Ok(None),
    }
  }

  fn resolve_id_filter(&self) -> anyhow::Result<Option<ResolvedIdHookFilter>> {
    match self.inner.resolve_id_filter {
      Some(ref item) => {
        let filter = ResolvedIdHookFilter::try_from(item.clone())?;
        Ok(Some(filter))
      }
      None => Ok(None),
    }
  }

  fn load_filter(&self) -> anyhow::Result<Option<LoadHookFilter>> {
    match self.inner.load_filter {
      Some(ref item) => {
        let filter = LoadHookFilter::try_from(item.clone())?;
        Ok(Some(filter))
      }
      None => Ok(None),
    }
  }
}
