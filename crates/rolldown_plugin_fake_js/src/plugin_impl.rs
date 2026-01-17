use std::borrow::Cow;
use std::sync::Arc;

use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkOutput, HookTransformArgs, HookTransformOutput, HookUsage,
  Plugin, PluginContext, SharedTransformPluginContext,
};

use crate::{transform::FakeJsPlugin, types::FakeJsOptions};

#[derive(Debug)]
pub struct FakeJsRolldownPlugin {
  inner: Arc<FakeJsPlugin>,
}

impl FakeJsRolldownPlugin {
  pub fn new(options: FakeJsOptions) -> Self {
    Self { inner: Arc::new(FakeJsPlugin::new(options)) }
  }
}

impl Plugin for FakeJsRolldownPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:fake-js")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::RenderChunk
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !args.id.ends_with(".d.ts") && !args.id.ends_with(".d.mts") && !args.id.ends_with(".d.cts") {
      return Ok(None);
    }

    match self.inner.transform(args.code, args.id) {
      Ok(result) => {
        Ok(Some(HookTransformOutput { code: Some(result.code), map: None, ..Default::default() }))
      }
      Err(e) => Err(anyhow::anyhow!("FakeJs transform error: {e}")),
    }
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if !args.chunk.filename.ends_with(".d.ts")
      && !args.chunk.filename.ends_with(".d.mts")
      && !args.chunk.filename.ends_with(".d.cts")
    {
      return Ok(None);
    }

    let chunk_info = crate::types::ChunkInfo {
      filename: args.chunk.filename.to_string(),
      module_ids: args.chunk.module_ids.iter().map(std::string::ToString::to_string).collect(),
    };

    match self.inner.render_chunk(&args.code, &chunk_info) {
      Ok(code) => Ok(Some(HookRenderChunkOutput { code, map: None })),
      Err(e) => Err(anyhow::anyhow!("FakeJs render_chunk error: {e}")),
    }
  }
}
