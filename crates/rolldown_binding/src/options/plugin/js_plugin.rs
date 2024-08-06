use crate::types::{
  binding_module_info::BindingModuleInfo, binding_outputs::BindingOutputs,
  js_callback::MaybeAsyncJsCallbackExt,
};
use rolldown_plugin::{Plugin, __inner::SharedPluginable, typedmap::TypedMapKey};
use std::{borrow::Cow, ops::Deref, sync::Arc};

use super::{
  binding_transform_context::BindingTransformPluginContext,
  types::binding_hook_resolve_id_extra_options::BindingHookResolveIdExtraOptions,
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
    ctx: &rolldown_plugin::SharedPluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_start {
      cb.await_call(Arc::clone(ctx).into()).await?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if let Some(cb) = &self.resolve_id {
      let custom = args
        .custom
        .get::<JsPluginContextResolveCustomArgId>(&JsPluginContextResolveCustomArgId)
        .map(|v| *v);
      Ok(
        cb.await_call((
          Arc::clone(ctx).into(),
          args.specifier.to_string(),
          args.importer.map(str::to_string),
          BindingHookResolveIdExtraOptions {
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

  async fn resolve_dynamic_import(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if let Some(cb) = &self.resolve_dynamic_import {
      Ok(
        cb.await_call((
          Arc::clone(ctx).into(),
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

  async fn load(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if let Some(cb) = &self.load {
      Ok(
        cb.await_call((Arc::clone(ctx).into(), args.id.to_string()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
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
        ))
        .await?
        .map(TryInto::try_into)
        .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  async fn module_parsed(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    module_info: Arc<rolldown_common::ModuleInfo>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.module_parsed {
      cb.await_call((Arc::clone(ctx).into(), BindingModuleInfo::new(module_info))).await?;
    }
    Ok(())
  }

  async fn build_end(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_end {
      cb.await_call((Arc::clone(ctx).into(), args.map(|a| a.error.to_string()))).await?;
    }
    Ok(())
  }

  // --- Generate hooks ---

  async fn render_start(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.render_start {
      cb.await_call(Arc::clone(ctx).into()).await?;
    }
    Ok(())
  }

  async fn banner(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookInjectionArgs<'_>,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if let Some(cb) = &self.banner {
      Ok(
        cb.await_call((Arc::clone(ctx).into(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  async fn intro(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookInjectionArgs<'_>,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if let Some(cb) = &self.intro {
      Ok(
        cb.await_call((Arc::clone(ctx).into(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  async fn outro(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookInjectionArgs<'_>,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if let Some(cb) = &self.outro {
      Ok(
        cb.await_call((Arc::clone(ctx).into(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  async fn footer(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookInjectionArgs<'_>,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if let Some(cb) = &self.footer {
      Ok(
        cb.await_call((Arc::clone(ctx).into(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if let Some(cb) = &self.render_chunk {
      Ok(
        cb.await_call((Arc::clone(ctx).into(), args.code.to_string(), args.chunk.clone().into()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  async fn augment_chunk_hash(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    chunk: &rolldown_common::RollupRenderedChunk,
  ) -> rolldown_plugin::HookAugmentChunkHashReturn {
    if let Some(cb) = &self.augment_chunk_hash {
      Ok(cb.await_call((Arc::clone(ctx).into(), chunk.clone().into())).await?)
    } else {
      Ok(None)
    }
  }

  async fn render_error(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookRenderErrorArgs,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.render_error {
      cb.await_call((Arc::clone(ctx).into(), args.error.to_string())).await?;
    }
    Ok(())
  }

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &mut Vec<rolldown_common::Output>,
    is_write: bool,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.generate_bundle {
      cb.await_call((
        Arc::clone(ctx).into(),
        BindingOutputs::new(unsafe { std::mem::transmute(bundle) }),
        is_write,
      ))
      .await?;
    }
    Ok(())
  }

  async fn write_bundle(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &mut Vec<rolldown_common::Output>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.write_bundle {
      cb.await_call((
        Arc::clone(ctx).into(),
        BindingOutputs::new(unsafe { std::mem::transmute(bundle) }),
      ))
      .await?;
    }
    Ok(())
  }
}
