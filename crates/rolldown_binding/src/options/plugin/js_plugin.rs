use std::{borrow::Cow, ops::Deref, sync::Arc};

use crate::utils::js_async_callback_ext::JsAsyncCallbackExt;
use rolldown_plugin::Plugin;

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
    if let Some(cb) = &self.build_start {
      cb.call_async_normalized(Arc::clone(ctx).into()).await?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookResolveIdArgs,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if let Some(cb) = &self.resolve_id {
      Ok(
        cb.call_async_normalized((
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
    if let Some(cb) = &self.load {
      Ok(cb.call_async_normalized(args.id.to_string()).await?.map(TryInto::try_into).transpose()?)
    } else {
      Ok(None)
    }
  }

  async fn transform(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookTransformArgs,
  ) -> rolldown_plugin::HookTransformReturn {
    if let Some(cb) = &self.transform {
      Ok(
        cb.call_async_normalized((args.code.to_string(), args.id.to_string()))
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
    if let Some(cb) = &self.build_end {
      cb.call_async_normalized(args.map(|a| a.error.to_string())).await?;
    }
    Ok(())
  }

  async fn render_chunk(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if let Some(cb) = &self.render_chunk {
      Ok(
        cb.call_async_normalized((args.code.to_string(), args.chunk.clone().into()))
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
    if let Some(cb) = &self.generate_bundle {
      cb.call_async_normalized((bundle.clone().into(), is_write)).await?;
    }
    Ok(())
  }

  async fn write_bundle(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &Vec<rolldown_common::Output>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.write_bundle {
      cb.call_async_normalized(bundle.clone().into()).await?;
    }
    Ok(())
  }
}
