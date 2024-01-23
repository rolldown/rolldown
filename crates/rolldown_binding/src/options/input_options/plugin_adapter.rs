use std::borrow::Cow;

use crate::utils::napi_error_ext::NapiErrorExt;
use crate::utils::JsCallback;
use derivative::Derivative;
use rolldown::Plugin;

use super::plugin::{
  HookRenderChunkOutput, HookResolveIdArgsOptions, PluginOptions, RenderedChunk, ResolveIdResult,
  SourceResult,
};

pub type BuildStartCallback = JsCallback<(), ()>;
pub type ResolveIdCallback =
  JsCallback<(String, Option<String>, HookResolveIdArgsOptions), Option<ResolveIdResult>>;
pub type LoadCallback = JsCallback<(String,), Option<SourceResult>>;
pub type TransformCallback = JsCallback<(String, String), Option<SourceResult>>;
pub type BuildEndCallback = JsCallback<(Option<String>,), ()>;
pub type RenderChunkCallback = JsCallback<(String, RenderedChunk), Option<HookRenderChunkOutput>>;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct JsAdapterPlugin {
  pub name: String,
  #[derivative(Debug = "ignore")]
  build_start_fn: Option<BuildStartCallback>,
  #[derivative(Debug = "ignore")]
  resolve_id_fn: Option<ResolveIdCallback>,
  #[derivative(Debug = "ignore")]
  load_fn: Option<LoadCallback>,
  #[derivative(Debug = "ignore")]
  transform_fn: Option<TransformCallback>,
  #[derivative(Debug = "ignore")]
  build_end_fn: Option<BuildEndCallback>,
  #[derivative(Debug = "ignore")]
  render_chunk_fn: Option<RenderChunkCallback>,
}

impl JsAdapterPlugin {
  pub fn new(option: PluginOptions) -> napi::Result<Self> {
    let build_start_fn = option.build_start.as_ref().map(BuildStartCallback::new).transpose()?;
    let resolve_id_fn = option.resolve_id.as_ref().map(ResolveIdCallback::new).transpose()?;
    let load_fn = option.load.as_ref().map(LoadCallback::new).transpose()?;
    let transform_fn = option.transform.as_ref().map(TransformCallback::new).transpose()?;
    let build_end_fn = option.build_end.as_ref().map(BuildEndCallback::new).transpose()?;
    let render_chunk_fn = option.render_chunk.as_ref().map(RenderChunkCallback::new).transpose()?;
    Ok(Self {
      name: option.name,
      build_start_fn,
      resolve_id_fn,
      load_fn,
      transform_fn,
      build_end_fn,
      render_chunk_fn,
    })
  }

  pub fn new_boxed(option: PluginOptions) -> napi::Result<Box<dyn Plugin>> {
    Ok(Box::new(Self::new(option)?))
  }
}

#[async_trait::async_trait]
impl Plugin for JsAdapterPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Owned(self.name.to_string())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn build_start(&self, _ctx: &mut rolldown::PluginContext) -> rolldown::HookNoopReturn {
    if let Some(cb) = &self.build_start_fn {
      cb.call_async(()).await.map_err(|e| e.into_bundle_error())?;
    }
    Ok(())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn resolve_id(
    &self,
    _ctx: &mut rolldown::PluginContext,
    args: &rolldown::HookResolveIdArgs,
  ) -> rolldown::HookResolveIdReturn {
    if let Some(cb) = &self.resolve_id_fn {
      let res = cb
        .call_async((
          args.source.to_string(),
          args.importer.map(|s| s.to_string()),
          args.options.clone().into(),
        ))
        .await
        .map_err(|e| e.into_bundle_error())?;

      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn load(
    &self,
    _ctx: &mut rolldown::PluginContext,
    args: &rolldown::HookLoadArgs,
  ) -> rolldown::HookLoadReturn {
    if let Some(cb) = &self.load_fn {
      let res = cb.call_async((args.id.to_string(),)).await.map_err(|e| e.into_bundle_error())?;
      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn transform(
    &self,
    _ctx: &mut rolldown::PluginContext,
    args: &rolldown::HookTransformArgs,
  ) -> rolldown::HookTransformReturn {
    if let Some(cb) = &self.transform_fn {
      let res = cb
        .call_async((args.code.to_string(), args.id.to_string()))
        .await
        .map_err(|e| e.into_bundle_error())?;
      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn build_end(
    &self,
    _ctx: &mut rolldown::PluginContext,
    args: Option<&rolldown::HookBuildEndArgs>,
  ) -> rolldown::HookNoopReturn {
    if let Some(cb) = &self.build_end_fn {
      cb.call_async((args.map(|a| a.error.to_string()),))
        .await
        .map_err(|e| e.into_bundle_error())?;
    }
    Ok(())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn render_chunk(
    &self,
    _ctx: &rolldown::PluginContext,
    args: &rolldown::RenderChunkArgs,
  ) -> rolldown::HookRenderChunkReturn {
    if let Some(cb) = &self.render_chunk_fn {
      let res = cb
        .call_async((args.code.to_string(), args.chunk.clone().into()))
        .await
        .map_err(|e| e.into_bundle_error())?;
      return Ok(res.map(Into::into));
    }
    Ok(None)
  }
}
