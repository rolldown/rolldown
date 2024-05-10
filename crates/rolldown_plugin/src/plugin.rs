use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use super::plugin_context::SharedPluginContext;
use crate::{
  types::hook_render_error::HookRenderErrorArgs, HookBuildEndArgs, HookLoadArgs, HookLoadOutput,
  HookRenderChunkArgs, HookRenderChunkOutput, HookResolveDynamicImportArgs, HookResolveIdArgs,
  HookResolveIdOutput, HookTransformArgs,
};
use anyhow::Result;
use rolldown_common::{ModuleInfo, Output};

pub type HookResolveIdReturn = Result<Option<HookResolveIdOutput>>;
pub type HookTransformReturn = Result<Option<HookLoadOutput>>;
pub type HookLoadReturn = Result<Option<HookLoadOutput>>;
pub type HookNoopReturn = Result<()>;
pub type HookRenderChunkReturn = Result<Option<HookRenderChunkOutput>>;

#[async_trait::async_trait]
pub trait Plugin: Any + Debug + Send + Sync + 'static {
  fn name(&self) -> Cow<'static, str>;

  // The `option` hook consider call at node side.

  // --- Build hooks ---

  async fn build_start(&self, _ctx: &SharedPluginContext) -> HookNoopReturn {
    Ok(())
  }

  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    Ok(None)
  }

  async fn resolve_dynamic_import(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookResolveDynamicImportArgs,
  ) -> HookResolveIdReturn {
    Ok(None)
  }

  async fn load(&self, _ctx: &SharedPluginContext, _args: &HookLoadArgs) -> HookLoadReturn {
    Ok(None)
  }

  async fn transform(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookTransformArgs,
  ) -> HookTransformReturn {
    Ok(None)
  }

  async fn module_parsed(
    &self,
    _ctx: &SharedPluginContext,
    _module_info: Arc<ModuleInfo>,
  ) -> HookNoopReturn {
    Ok(())
  }

  async fn build_end(
    &self,
    _ctx: &SharedPluginContext,
    _args: Option<&HookBuildEndArgs>,
  ) -> HookNoopReturn {
    Ok(())
  }

  // --- Generate hooks ---

  #[allow(clippy::ptr_arg)]
  async fn render_start(&self, _ctx: &SharedPluginContext) -> HookNoopReturn {
    Ok(())
  }

  async fn render_chunk(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderChunkArgs,
  ) -> HookRenderChunkReturn {
    Ok(None)
  }

  async fn render_error(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderErrorArgs,
  ) -> HookNoopReturn {
    Ok(())
  }

  #[allow(clippy::ptr_arg)]
  async fn generate_bundle(
    &self,
    _ctx: &SharedPluginContext,
    _bundle: &mut Vec<Output>,
    _is_write: bool,
  ) -> HookNoopReturn {
    Ok(())
  }

  #[allow(clippy::ptr_arg)]
  async fn write_bundle(
    &self,
    _ctx: &SharedPluginContext,
    _bundle: &mut Vec<Output>,
  ) -> HookNoopReturn {
    Ok(())
  }
}

pub type BoxPlugin = Box<dyn Plugin>;
