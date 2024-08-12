use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use super::plugin_context::PluginContext;
use crate::{
  plugin_hook_meta::PluginHookMeta,
  transform_plugin_context::TransformPluginContext,
  types::{hook_render_error::HookRenderErrorArgs, hook_transform_ast_args::HookTransformAstArgs},
  HookAddonArgs, HookBuildEndArgs, HookInjectionOutputReturn, HookLoadArgs, HookRenderChunkArgs,
  HookResolveIdArgs, HookTransformArgs, Plugin,
};
use rolldown_common::{ModuleInfo, Output, RollupRenderedChunk};

pub use crate::plugin::HookAugmentChunkHashReturn;
pub use crate::plugin::HookLoadReturn;
pub use crate::plugin::HookNoopReturn;
pub use crate::plugin::HookRenderChunkReturn;
pub use crate::plugin::HookResolveIdReturn;
pub use crate::plugin::HookTransformAstReturn;
pub use crate::plugin::HookTransformReturn;

pub type BoxPluginable = Box<dyn Pluginable>;
pub type SharedPluginable = Arc<dyn Pluginable>;

/// `Pluginable` is under the hood trait that rolldown to run. It's not recommended to use this trait directly.
/// To create a plugin, you should use [Plugin] trait instead.
///
/// The main reason we don't expose this trait is that it used `async_trait`, which make it rust-analyzer can't
/// provide a good auto-completion experience.
#[async_trait::async_trait]
pub trait Pluginable: Any + Debug + Send + Sync + 'static {
  fn call_name(&self) -> Cow<'static, str>;

  // The `option` hook consider call at node side.

  // --- Build hooks ---

  async fn call_build_start(&self, _ctx: &PluginContext) -> HookNoopReturn;

  fn call_build_start_meta(&self) -> Option<PluginHookMeta>;

  async fn call_resolve_id(
    &self,
    _ctx: &PluginContext,
    _args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn;

  fn call_resolve_id_meta(&self) -> Option<PluginHookMeta>;

  #[deprecated(
    note = "This hook is only for rollup compatibility, please use `resolve_id` instead."
  )]
  async fn call_resolve_dynamic_import(
    &self,
    _ctx: &PluginContext,
    _args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn;

  fn call_resolve_dynamic_import_meta(&self) -> Option<PluginHookMeta>;

  async fn call_load(&self, _ctx: &PluginContext, _args: &HookLoadArgs) -> HookLoadReturn;

  fn call_load_meta(&self) -> Option<PluginHookMeta>;

  async fn call_transform(
    &self,
    _ctx: &TransformPluginContext<'_>,
    _args: &HookTransformArgs,
  ) -> HookTransformReturn;

  fn call_transform_meta(&self) -> Option<PluginHookMeta>;

  fn call_transform_ast(
    &self,
    _ctx: &PluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn;

  fn call_transform_ast_meta(&self) -> Option<PluginHookMeta>;

  async fn call_module_parsed(
    &self,
    _ctx: &PluginContext,
    _module_info: Arc<ModuleInfo>,
  ) -> HookNoopReturn;

  fn call_module_parsed_meta(&self) -> Option<PluginHookMeta>;

  async fn call_build_end(
    &self,
    _ctx: &PluginContext,
    _args: Option<&HookBuildEndArgs>,
  ) -> HookNoopReturn;

  fn call_build_end_meta(&self) -> Option<PluginHookMeta>;

  // --- Generate hooks ---

  async fn call_render_start(&self, _ctx: &PluginContext) -> HookNoopReturn;

  fn call_render_start_meta(&self) -> Option<PluginHookMeta>;

  async fn call_banner(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn;

  fn call_banner_meta(&self) -> Option<PluginHookMeta>;

  async fn call_footer(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn;

  fn call_footer_meta(&self) -> Option<PluginHookMeta>;

  async fn call_intro(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn;

  fn call_intro_meta(&self) -> Option<PluginHookMeta>;

  async fn call_outro(
    &self,
    _ctx: &PluginContext,
    _args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn;

  fn call_outro_meta(&self) -> Option<PluginHookMeta>;

  async fn call_render_chunk(
    &self,
    _ctx: &PluginContext,
    _args: &HookRenderChunkArgs,
  ) -> HookRenderChunkReturn;

  fn call_render_chunk_meta(&self) -> Option<PluginHookMeta>;

  async fn call_augment_chunk_hash(
    &self,
    _ctx: &PluginContext,
    _chunk: &RollupRenderedChunk,
  ) -> HookAugmentChunkHashReturn;

  fn call_augment_chunk_hash_meta(&self) -> Option<PluginHookMeta>;

  async fn call_render_error(
    &self,
    _ctx: &PluginContext,
    _args: &HookRenderErrorArgs,
  ) -> HookNoopReturn;

  fn call_render_error_meta(&self) -> Option<PluginHookMeta>;

  async fn call_generate_bundle(
    &self,
    _ctx: &PluginContext,
    _bundle: &mut Vec<Output>,
    _is_write: bool,
  ) -> HookNoopReturn;

  fn call_generate_bundle_meta(&self) -> Option<PluginHookMeta>;

  async fn call_write_bundle(
    &self,
    _ctx: &PluginContext,
    _bundle: &mut Vec<Output>,
  ) -> HookNoopReturn;

  fn call_write_bundle_meta(&self) -> Option<PluginHookMeta>;
}

#[async_trait::async_trait]
impl<T: Plugin> Pluginable for T {
  fn call_name(&self) -> Cow<'static, str> {
    Plugin::name(self)
  }

  async fn call_build_start(&self, ctx: &PluginContext) -> HookNoopReturn {
    Plugin::build_start(self, ctx).await
  }

  fn call_build_start_meta(&self) -> Option<PluginHookMeta> {
    Plugin::build_start_meta(self)
  }

  async fn call_resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    Plugin::resolve_id(self, ctx, args).await
  }

  fn call_resolve_id_meta(&self) -> Option<PluginHookMeta> {
    Plugin::resolve_id_meta(self)
  }

  #[allow(deprecated)]
  async fn call_resolve_dynamic_import(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    Plugin::resolve_dynamic_import(self, ctx, args).await
  }

  fn call_resolve_dynamic_import_meta(&self) -> Option<PluginHookMeta> {
    Plugin::resolve_dynamic_import_meta(self)
  }

  async fn call_load(&self, ctx: &PluginContext, args: &HookLoadArgs) -> HookLoadReturn {
    Plugin::load(self, ctx, args).await
  }

  fn call_load_meta(&self) -> Option<PluginHookMeta> {
    Plugin::load_meta(self)
  }

  async fn call_transform(
    &self,
    ctx: &TransformPluginContext<'_>,
    args: &HookTransformArgs,
  ) -> HookTransformReturn {
    Plugin::transform(self, ctx, args).await
  }

  fn call_transform_meta(&self) -> Option<PluginHookMeta> {
    Plugin::transform_meta(self)
  }

  async fn call_module_parsed(
    &self,
    ctx: &PluginContext,
    module_info: Arc<ModuleInfo>,
  ) -> HookNoopReturn {
    Plugin::module_parsed(self, ctx, module_info).await
  }

  fn call_module_parsed_meta(&self) -> Option<PluginHookMeta> {
    Plugin::module_parsed_meta(self)
  }

  async fn call_build_end(
    &self,
    ctx: &PluginContext,
    args: Option<&HookBuildEndArgs>,
  ) -> HookNoopReturn {
    Plugin::build_end(self, ctx, args).await
  }

  fn call_build_end_meta(&self) -> Option<PluginHookMeta> {
    Plugin::build_end_meta(self)
  }

  async fn call_render_start(&self, ctx: &PluginContext) -> HookNoopReturn {
    Plugin::render_start(self, ctx).await
  }

  fn call_render_start_meta(&self) -> Option<PluginHookMeta> {
    Plugin::render_start_meta(self)
  }

  async fn call_banner(
    &self,
    ctx: &PluginContext,
    args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn {
    Plugin::banner(self, ctx, args).await
  }

  fn call_banner_meta(&self) -> Option<PluginHookMeta> {
    Plugin::banner_meta(self)
  }

  async fn call_footer(
    &self,
    ctx: &PluginContext,
    args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn {
    Plugin::footer(self, ctx, args).await
  }

  fn call_footer_meta(&self) -> Option<PluginHookMeta> {
    Plugin::footer_meta(self)
  }

  async fn call_intro(
    &self,
    ctx: &PluginContext,
    args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn {
    Plugin::intro(self, ctx, args).await
  }

  fn call_intro_meta(&self) -> Option<PluginHookMeta> {
    Plugin::intro_meta(self)
  }

  async fn call_outro(
    &self,
    ctx: &PluginContext,
    args: &HookAddonArgs,
  ) -> HookInjectionOutputReturn {
    Plugin::outro(self, ctx, args).await
  }

  fn call_outro_meta(&self) -> Option<PluginHookMeta> {
    Plugin::outro_meta(self)
  }

  async fn call_render_chunk(
    &self,
    ctx: &PluginContext,
    args: &HookRenderChunkArgs,
  ) -> HookRenderChunkReturn {
    Plugin::render_chunk(self, ctx, args).await
  }

  fn call_render_chunk_meta(&self) -> Option<PluginHookMeta> {
    Plugin::render_chunk_meta(self)
  }

  async fn call_augment_chunk_hash(
    &self,
    ctx: &PluginContext,
    chunk: &RollupRenderedChunk,
  ) -> HookAugmentChunkHashReturn {
    Plugin::augment_chunk_hash(self, ctx, chunk).await
  }

  fn call_augment_chunk_hash_meta(&self) -> Option<PluginHookMeta> {
    Plugin::augment_chunk_hash_meta(self)
  }

  async fn call_render_error(
    &self,
    ctx: &PluginContext,
    args: &HookRenderErrorArgs,
  ) -> HookNoopReturn {
    Plugin::render_error(self, ctx, args).await
  }

  fn call_render_error_meta(&self) -> Option<PluginHookMeta> {
    Plugin::render_error_meta(self)
  }

  async fn call_generate_bundle(
    &self,
    ctx: &PluginContext,
    bundle: &mut Vec<Output>,
    is_write: bool,
  ) -> HookNoopReturn {
    Plugin::generate_bundle(self, ctx, bundle, is_write).await
  }

  fn call_generate_bundle_meta(&self) -> Option<PluginHookMeta> {
    Plugin::generate_bundle_meta(self)
  }

  async fn call_write_bundle(
    &self,
    ctx: &PluginContext,
    bundle: &mut Vec<Output>,
  ) -> HookNoopReturn {
    Plugin::write_bundle(self, ctx, bundle).await
  }

  fn call_write_bundle_meta(&self) -> Option<PluginHookMeta> {
    Plugin::write_bundle_meta(self)
  }

  fn call_transform_ast(
    &self,
    ctx: &PluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    Plugin::transform_ast(self, ctx, args)
  }

  fn call_transform_ast_meta(&self) -> Option<PluginHookMeta> {
    Plugin::transform_ast_meta(self)
  }
}
