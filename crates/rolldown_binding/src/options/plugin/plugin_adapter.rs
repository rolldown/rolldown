use std::borrow::Cow;

use crate::utils::JsCallback;
use crate::{types::binding_outputs::BindingOutputs, utils::napi_error_ext::NapiErrorExt};
use derivative::Derivative;
use rolldown_plugin::Plugin;

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
pub type GenerateBundleCallback = JsCallback<(BindingOutputs, bool), Option<HookRenderChunkOutput>>;
pub type WriteBundleCallback = JsCallback<(BindingOutputs,), ()>;

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
  #[derivative(Debug = "ignore")]
  generate_bundle_fn: Option<GenerateBundleCallback>,
  #[derivative(Debug = "ignore")]
  write_bundle_fn: Option<WriteBundleCallback>,
}

impl JsAdapterPlugin {
  pub fn new(option: PluginOptions) -> napi::Result<Self> {
    let build_start_fn = option.build_start.as_ref().map(BuildStartCallback::new).transpose()?;
    let resolve_id_fn = option.resolve_id.as_ref().map(ResolveIdCallback::new).transpose()?;
    let load_fn = option.load.as_ref().map(LoadCallback::new).transpose()?;
    let transform_fn = option.transform.as_ref().map(TransformCallback::new).transpose()?;
    let build_end_fn = option.build_end.as_ref().map(BuildEndCallback::new).transpose()?;
    let render_chunk_fn = option.render_chunk.as_ref().map(RenderChunkCallback::new).transpose()?;
    let generate_bundle_fn =
      option.generate_bundle.as_ref().map(GenerateBundleCallback::new).transpose()?;
    let write_bundle_fn = option.write_bundle.as_ref().map(WriteBundleCallback::new).transpose()?;
    Ok(Self {
      name: option.name,
      build_start_fn,
      resolve_id_fn,
      load_fn,
      transform_fn,
      build_end_fn,
      render_chunk_fn,
      generate_bundle_fn,
      write_bundle_fn,
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
  async fn build_start(
    &self,
    _ctx: &mut rolldown_plugin::PluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_start_fn {
      cb.call_async(()).await.map_err(|e| e.into_bundle_error())?;
    }
    Ok(())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn resolve_id(
    &self,
    _ctx: &mut rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs,
  ) -> rolldown_plugin::HookResolveIdReturn {
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
    _ctx: &mut rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs,
  ) -> rolldown_plugin::HookLoadReturn {
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
    _ctx: &mut rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookTransformArgs,
  ) -> rolldown_plugin::HookTransformReturn {
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
    _ctx: &mut rolldown_plugin::PluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs>,
  ) -> rolldown_plugin::HookNoopReturn {
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
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::RenderChunkArgs,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if let Some(cb) = &self.render_chunk_fn {
      let res = cb
        .call_async((args.code.to_string(), args.chunk.clone().into()))
        .await
        .map_err(|e| e.into_bundle_error())?;
      return Ok(res.map(Into::into));
    }
    Ok(None)
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    bundle: &Vec<rolldown_common::Output>,
    is_write: bool,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.generate_bundle_fn {
      cb.call_async((bundle.clone().into(), is_write)).await.map_err(|e| e.into_bundle_error())?;
    }
    Ok(())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn write_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    bundle: &Vec<rolldown_common::Output>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.write_bundle_fn {
      cb.call_async((bundle.clone().into(),)).await.map_err(|e| e.into_bundle_error())?;
    }
    Ok(())
  }
}
