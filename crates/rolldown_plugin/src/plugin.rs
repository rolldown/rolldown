use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use super::plugin_context::PluginContext;
use crate::{
  HookAddonArgs, HookBuildEndArgs, HookCloseBundleArgs, HookGenerateBundleArgs, HookLoadArgs,
  HookLoadOutput, HookRenderChunkArgs, HookRenderChunkOutput, HookResolveIdArgs,
  HookResolveIdOutput, HookTransformArgs, HookUsage, HookWriteBundleArgs, PluginHookMeta,
  SharedLoadPluginContext, SharedTransformPluginContext,
  types::{
    hook_build_start_args::HookBuildStartArgs, hook_render_error::HookRenderErrorArgs,
    hook_render_start_args::HookRenderStartArgs, hook_transform_ast_args::HookTransformAstArgs,
    hook_transform_output::HookTransformOutput,
  },
};
use anyhow::Result;
use rolldown_common::{ModuleInfo, NormalModule, RollupRenderedChunk, WatcherChangeKind};
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

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn build_start_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    _args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok(None)
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  #[deprecated(
    note = "This hook is only for rollup compatibility, please use `resolve_id` instead."
  )]
  async fn resolve_dynamic_import(
    &self,
    _ctx: &PluginContext,
    _args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok(None)
  }

  fn resolve_dynamic_import_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn load(
    &self,
    _ctx: SharedLoadPluginContext,
    _args: &HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    Ok(None)
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    _args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    Ok(None)
  }

  fn transform_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn module_parsed(
    &self,
    _ctx: &PluginContext,
    _module_info: Arc<ModuleInfo>,
    _normal_module: &NormalModule,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn module_parsed_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn build_end(
    &self,
    _ctx: &PluginContext,
    _args: Option<&HookBuildEndArgs<'_>>,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn build_end_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  // --- Generate hooks ---

  async fn render_start(
    &self,
    _ctx: &PluginContext,
    _args: &HookRenderStartArgs<'_>,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn render_start_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn banner(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn {
    Ok(None)
  }

  fn banner_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn footer(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn {
    Ok(None)
  }

  fn footer_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn intro(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn {
    Ok(None)
  }

  fn intro_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn outro(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn {
    Ok(None)
  }

  fn outro_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    _args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    Ok(None)
  }

  fn render_chunk_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn augment_chunk_hash(
    &self,
    _ctx: &PluginContext,
    _chunk: Arc<RollupRenderedChunk>,
  ) -> HookAugmentChunkHashReturn {
    Ok(None)
  }

  fn augment_chunk_hash_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn render_error(
    &self,
    _ctx: &PluginContext,
    _args: &HookRenderErrorArgs<'_>,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn render_error_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    _args: &mut HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn generate_bundle_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn write_bundle(
    &self,
    _ctx: &PluginContext,
    _args: &mut HookWriteBundleArgs<'_>,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn write_bundle_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn close_bundle(
    &self,
    _ctx: &PluginContext,
    _args: Option<&HookCloseBundleArgs<'_>>,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn close_bundle_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  // watch hooks

  async fn watch_change(
    &self,
    _ctx: &PluginContext,
    _path: &str,
    _event: WatcherChangeKind,
  ) -> HookNoopReturn {
    Ok(())
  }

  fn watch_change_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  async fn close_watcher(&self, _ctx: &PluginContext) -> HookNoopReturn {
    Ok(())
  }

  fn close_watcher_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  // --- experimental hooks ---
  async fn transform_ast(
    &self,
    _ctx: &PluginContext,
    args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    Ok(args.ast)
  }

  fn transform_ast_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn register_hook_usage(&self) -> HookUsage;
}
