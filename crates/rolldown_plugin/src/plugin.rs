use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use super::plugin_context::SharedPluginContext;
use crate::{
  transform_plugin_context::TransformPluginContext,
  types::{
    hook_render_error::HookRenderErrorArgs, hook_transform_ast_args::HookTransformAstArgs,
    hook_transform_output::HookTransformOutput,
  },
  HookBuildEndArgs, HookInjectionArgs, HookLoadArgs, HookLoadOutput, HookRenderChunkArgs,
  HookRenderChunkOutput, HookResolveDynamicImportArgs, HookResolveIdArgs, HookResolveIdOutput,
  HookTransformArgs,
};
use anyhow::Result;
use rolldown_common::{ModuleInfo, Output, RollupRenderedChunk};
use rolldown_ecmascript::EcmaAst;

pub type HookResolveIdReturn = Result<Option<HookResolveIdOutput>>;
pub type HookTransformAstReturn = Result<EcmaAst>;
pub type HookTransformReturn = Result<Option<HookTransformOutput>>;
pub type HookLoadReturn = Result<Option<HookLoadOutput>>;
pub type HookNoopReturn = Result<()>;
pub type HookRenderChunkReturn = Result<Option<HookRenderChunkOutput>>;
pub type HookAugmentChunkHashReturn = Result<Option<String>>;
pub type HookInjectionOutputReturn = Result<Option<String>>;

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

  #[deprecated(
    note = "This hook is only for rollup compatibility, please use `resolve_id` instead."
  )]
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
    _ctx: &TransformPluginContext<'_>,
    _args: &HookTransformArgs,
  ) -> HookTransformReturn {
    Ok(None)
  }

  fn transform_ast(
    &self,
    _ctx: &SharedPluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    Ok(args.ast)
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

  async fn render_start(&self, _ctx: &SharedPluginContext) -> HookNoopReturn {
    Ok(())
  }

  // TODO use macros to
  async fn banner(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn {
    Ok(None)
  }

  async fn footer(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn {
    Ok(None)
  }

  async fn intro(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn {
    Ok(None)
  }

  async fn outro(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn {
    Ok(None)
  }

  async fn render_chunk(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderChunkArgs,
  ) -> HookRenderChunkReturn {
    Ok(None)
  }

  async fn augment_chunk_hash(
    &self,
    _ctx: &SharedPluginContext,
    _chunk: &RollupRenderedChunk,
  ) -> HookAugmentChunkHashReturn {
    Ok(None)
  }

  async fn render_error(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderErrorArgs,
  ) -> HookNoopReturn {
    Ok(())
  }

  async fn generate_bundle(
    &self,
    _ctx: &SharedPluginContext,
    _bundle: &mut Vec<Output>,
    _is_write: bool,
  ) -> HookNoopReturn {
    Ok(())
  }

  async fn write_bundle(
    &self,
    _ctx: &SharedPluginContext,
    _bundle: &mut Vec<Output>,
  ) -> HookNoopReturn {
    Ok(())
  }
}

pub type BoxPlugin = Box<dyn Plugin>;
pub type SharedPlugin = Arc<dyn Plugin>;
