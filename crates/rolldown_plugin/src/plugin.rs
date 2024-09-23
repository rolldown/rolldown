use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use super::plugin_context::PluginContext;
use crate::{
  plugin_hook_meta::PluginHookMeta,
  transform_plugin_context::TransformPluginContext,
  types::{
    hook_filter::{LoadHookFilter, ResolvedIdHookFilter, TransformHookFilter},
    hook_render_error::HookRenderErrorArgs,
    hook_transform_ast_args::HookTransformAstArgs,
    hook_transform_output::HookTransformOutput,
  },
  HookAddonArgs, HookBuildEndArgs, HookLoadArgs, HookLoadOutput, HookRenderChunkArgs,
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

pub trait Plugin: Any + Debug + Send + Sync + 'static {
  fn name(&self) -> Cow<'static, str>;

  // The `option` hook consider call at node side.

  // --- Build hooks ---

  fn build_start(
    &self,
    _ctx: &PluginContext,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn build_start_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn resolve_id(
    &self,
    _ctx: &PluginContext,
    _args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
    async { Ok(None) }
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  #[deprecated(
    note = "This hook is only for rollup compatibility, please use `resolve_id` instead."
  )]
  fn resolve_dynamic_import(
    &self,
    _ctx: &PluginContext,
    _args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
    async { Ok(None) }
  }

  fn resolve_dynamic_import_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn load(
    &self,
    _ctx: &PluginContext,
    _args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    async { Ok(None) }
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn transform(
    &self,
    _ctx: &TransformPluginContext<'_>,
    _args: &HookTransformArgs<'_>,
  ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
    async { Ok(None) }
  }

  fn transform_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn module_parsed(
    &self,
    _ctx: &PluginContext,
    _module_info: Arc<ModuleInfo>,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn module_parsed_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn build_end(
    &self,
    _ctx: &PluginContext,
    _args: Option<&HookBuildEndArgs>,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn build_end_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  // --- Generate hooks ---

  fn render_start(
    &self,
    _ctx: &PluginContext,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn render_start_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn banner(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs<'_>,
  ) -> impl std::future::Future<Output = HookInjectionOutputReturn> + Send {
    async { Ok(None) }
  }

  fn banner_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn footer(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs<'_>,
  ) -> impl std::future::Future<Output = HookInjectionOutputReturn> + Send {
    async { Ok(None) }
  }

  fn footer_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn intro(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs<'_>,
  ) -> impl std::future::Future<Output = HookInjectionOutputReturn> + Send {
    async { Ok(None) }
  }

  fn intro_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn outro(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs<'_>,
  ) -> impl std::future::Future<Output = HookInjectionOutputReturn> + Send {
    async { Ok(None) }
  }

  fn outro_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn render_chunk(
    &self,
    _ctx: &PluginContext,
    _args: &HookRenderChunkArgs<'_>,
  ) -> impl std::future::Future<Output = HookRenderChunkReturn> + Send {
    async { Ok(None) }
  }

  fn render_chunk_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn augment_chunk_hash(
    &self,
    _ctx: &PluginContext,
    _chunk: &RollupRenderedChunk,
  ) -> impl std::future::Future<Output = HookAugmentChunkHashReturn> + Send {
    async { Ok(None) }
  }

  fn augment_chunk_hash_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn render_error(
    &self,
    _ctx: &PluginContext,
    _args: &HookRenderErrorArgs,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn render_error_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    _bundle: &mut Vec<Output>,
    _is_write: bool,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn generate_bundle_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn write_bundle(
    &self,
    _ctx: &PluginContext,
    _bundle: &mut Vec<Output>,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn write_bundle_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn close_bundle(
    &self,
    _ctx: &PluginContext,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
    async { Ok(()) }
  }

  fn close_bundle_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  // --- experimental hooks ---

  fn transform_ast(
    &self,
    _ctx: &PluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    Ok(args.ast)
  }

  fn transform_ast_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn transform_filter(&self) -> anyhow::Result<Option<TransformHookFilter>> {
    Ok(None)
  }

  fn resolve_id_filter(&self) -> anyhow::Result<Option<ResolvedIdHookFilter>> {
    Ok(None)
  }

  fn load_filter(&self) -> anyhow::Result<Option<LoadHookFilter>> {
    Ok(None)
  }
}
