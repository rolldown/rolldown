use std::{borrow::Cow, fmt::Debug};

use rolldown_error::BuildError;

use crate::Output;

use super::{
  args::{HookBuildEndArgs, HookLoadArgs, HookResolveIdArgs, HookTransformArgs, RenderChunkArgs},
  context::PluginContext,
  output::{HookLoadOutput, HookRenderChunkOutput, HookResolveIdOutput},
};

pub type HookResolveIdReturn = Result<Option<HookResolveIdOutput>, BuildError>;
pub type HookTransformReturn = Result<Option<HookLoadOutput>, BuildError>;
pub type HookLoadReturn = Result<Option<HookLoadOutput>, BuildError>;
pub type HookNoopReturn = Result<(), BuildError>;
pub type HookRenderChunkReturn = Result<Option<HookRenderChunkOutput>, BuildError>;

#[async_trait::async_trait]
pub trait Plugin: Debug + Send + Sync {
  fn name(&self) -> Cow<'static, str>;

  // The `option` hook consider call at node side.

  async fn build_start(&self, _ctx: &mut PluginContext) -> HookNoopReturn {
    Ok(())
  }

  async fn resolve_id(
    &self,
    _ctx: &mut PluginContext,
    _args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    Ok(None)
  }

  async fn load(&self, _ctx: &mut PluginContext, _args: &HookLoadArgs) -> HookLoadReturn {
    Ok(None)
  }

  async fn transform(
    &self,
    _ctx: &mut PluginContext,
    _args: &HookTransformArgs,
  ) -> HookTransformReturn {
    Ok(None)
  }

  async fn build_end(
    &self,
    _ctx: &mut PluginContext,
    _args: Option<&HookBuildEndArgs>,
  ) -> HookNoopReturn {
    Ok(())
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    _args: &RenderChunkArgs,
  ) -> HookRenderChunkReturn {
    Ok(None)
  }

  #[allow(clippy::ptr_arg)]
  async fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    _bundle: &Vec<Output>,
    _is_write: bool,
  ) -> HookNoopReturn {
    Ok(())
  }
}

pub type BoxPlugin = Box<dyn Plugin>;
