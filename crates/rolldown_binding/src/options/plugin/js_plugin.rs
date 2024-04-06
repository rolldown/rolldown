use crate::types::{binding_outputs::BindingOutputs, js_callback::MaybeAsyncJsCallbackExt};
use rolldown_plugin::Plugin;
use std::{borrow::Cow, ops::Deref, sync::Arc};

use super::BindingPluginOptions;

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
  pub(crate) fn new_boxed(inner: BindingPluginOptions) -> Box<dyn Plugin> {
    Box::new(Self { inner })
  }
}

#[async_trait::async_trait]
impl Plugin for JsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Owned(self.name.clone())
  }

  // --- Build hooks ---

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(hook_option) = &self.build_start {
      hook_option.handler.await_call(Arc::clone(ctx).into()).await?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookResolveIdArgs,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if let Some(hook_option) = &self.resolve_id {
      Ok(
        hook_option
          .handler
          .await_call((
            args.source.to_string(),
            args.importer.map(str::to_string),
            args.options.clone().into(),
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
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookLoadArgs,
  ) -> rolldown_plugin::HookLoadReturn {
    if let Some(hook_option) = &self.load {
      Ok(
        hook_option
          .handler
          .await_call(args.id.to_string())
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
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookTransformArgs,
  ) -> rolldown_plugin::HookTransformReturn {
    if let Some(hook_option) = &self.transform {
      Ok(
        hook_option
          .handler
          .await_call((args.code.to_string(), args.id.to_string()))
          .await?
          .map(TryInto::try_into)
          .transpose()?,
      )
    } else {
      Ok(None)
    }
  }

  async fn build_end(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(hook_option) = &self.build_end {
      hook_option.handler.await_call(args.map(|a| a.error.to_string())).await?;
    }
    Ok(())
  }

  async fn render_chunk(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if let Some(hook_option) = &self.render_chunk {
      Ok(
        hook_option
          .handler
          .await_call((args.code.to_string(), args.chunk.clone().into()))
          .await?
          .map(Into::into),
      )
    } else {
      Ok(None)
    }
  }

  // --- Output hooks ---

  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &Vec<rolldown_common::Output>,
    is_write: bool,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(hook_option) = &self.generate_bundle {
      hook_option.handler.await_call((BindingOutputs::new(bundle.clone()), is_write)).await?;
    }
    Ok(())
  }

  async fn write_bundle(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &Vec<rolldown_common::Output>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(hook_option) = &self.write_bundle {
      hook_option.handler.await_call(BindingOutputs::new(bundle.clone())).await?;
    }
    Ok(())
  }
}
