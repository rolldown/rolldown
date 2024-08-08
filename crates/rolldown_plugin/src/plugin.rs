use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use super::plugin_context::SharedPluginContext;
use crate::{
  transform_plugin_context::TransformPluginContext,
  types::{
    hook_render_error::HookRenderErrorArgs, hook_transform_ast_args::HookTransformAstArgs,
    hook_transform_output::HookTransformOutput,
  },
  HookBuildEndArgs, HookInjectionArgs, HookLoadArgs, HookLoadOutput, HookRenderChunkArgs,
  HookRenderChunkOutput, HookResolveIdArgs, HookResolveIdOutput, HookTransformArgs,
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
pub type HookGenerateBundleReturn = Result<Vec<Output>>;
pub type HookWriteBundleReturn = Result<Vec<Output>>;

pub trait Plugin: Any + Debug + Send + Sync + 'static {
  fn name(&self) -> Cow<'static, str>;

  // The `option` hook consider call at node side.

  // --- Build hooks ---

  fn build_start(
    &self,
    _ctx: &SharedPluginContext,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
    async { Ok(None) }
  }

  #[deprecated(
    note = "This hook is only for rollup compatibility, please use `resolve_id` instead."
  )]
  fn resolve_dynamic_import(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
    async { Ok(None) }
  }

  fn load(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    async { Ok(None) }
  }

  fn transform(
    &self,
    _ctx: &TransformPluginContext<'_>,
    _args: &HookTransformArgs<'_>,
  ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
    async { Ok(None) }
  }

  fn module_parsed(
    &self,
    _ctx: &SharedPluginContext,
    _module_info: Arc<ModuleInfo>,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn build_end(
    &self,
    _ctx: &SharedPluginContext,
    _args: Option<&HookBuildEndArgs>,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  // --- Generate hooks ---

  fn render_start(
    &self,
    _ctx: &SharedPluginContext,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn banner(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs<'_>,
  ) -> impl std::future::Future<Output = HookInjectionOutputReturn> + Send {
    async { Ok(None) }
  }

  fn footer(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs<'_>,
  ) -> impl std::future::Future<Output = HookInjectionOutputReturn> + Send {
    async { Ok(None) }
  }

  fn intro(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs<'_>,
  ) -> impl std::future::Future<Output = HookInjectionOutputReturn> + Send {
    async { Ok(None) }
  }

  fn outro(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs<'_>,
  ) -> impl std::future::Future<Output = HookInjectionOutputReturn> + Send {
    async { Ok(None) }
  }

  fn render_chunk(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderChunkArgs<'_>,
  ) -> impl std::future::Future<Output = HookRenderChunkReturn> + Send {
    async { Ok(None) }
  }

  fn augment_chunk_hash(
    &self,
    _ctx: &SharedPluginContext,
    _chunk: &RollupRenderedChunk,
  ) -> impl std::future::Future<Output = HookAugmentChunkHashReturn> + Send {
    async { Ok(None) }
  }

  fn render_error(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderErrorArgs,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn generate_bundle(
    &self,
    _ctx: &SharedPluginContext,
    bundle: Vec<Output>,
    _is_write: bool,
  ) -> impl std::future::Future<Output = HookGenerateBundleReturn> + Send {
    async { Ok(bundle) }
  }

  fn write_bundle(
    &self,
    _ctx: &SharedPluginContext,
    bundle: Vec<Output>,
  ) -> impl std::future::Future<Output = HookWriteBundleReturn> + Send {
    async { Ok(bundle) }
  }

  // --- experimental hooks ---

  fn transform_ast(
    &self,
    _ctx: &SharedPluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    Ok(args.ast)
  }
}
