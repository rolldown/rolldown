use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use super::plugin_context::SharedPluginContext;
use crate::{
  transform_plugin_context::TransformPluginContext,
  types::{hook_render_error::HookRenderErrorArgs, hook_transform_ast_args::HookTransformAstArgs},
  HookBuildEndArgs, HookGenerateBundleReturn, HookInjectionArgs, HookInjectionOutputReturn,
  HookLoadArgs, HookRenderChunkArgs, HookResolveIdArgs, HookTransformArgs, HookWriteBundleReturn,
  Plugin,
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

  async fn call_build_start(&self, _ctx: &SharedPluginContext) -> HookNoopReturn;

  async fn call_resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn;

  #[deprecated(
    note = "This hook is only for rollup compatibility, please use `resolve_id` instead."
  )]
  async fn call_resolve_dynamic_import(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn;

  async fn call_load(&self, _ctx: &SharedPluginContext, _args: &HookLoadArgs) -> HookLoadReturn;

  async fn call_transform(
    &self,
    _ctx: &TransformPluginContext<'_>,
    _args: &HookTransformArgs,
  ) -> HookTransformReturn;

  fn call_transform_ast(
    &self,
    _ctx: &SharedPluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn;

  async fn call_module_parsed(
    &self,
    _ctx: &SharedPluginContext,
    _module_info: Arc<ModuleInfo>,
  ) -> HookNoopReturn;

  async fn call_build_end(
    &self,
    _ctx: &SharedPluginContext,
    _args: Option<&HookBuildEndArgs>,
  ) -> HookNoopReturn;

  // --- Generate hooks ---

  async fn call_render_start(&self, _ctx: &SharedPluginContext) -> HookNoopReturn;

  async fn call_banner(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn;

  async fn call_footer(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn;

  async fn call_intro(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn;

  async fn call_outro(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn;

  async fn call_render_chunk(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderChunkArgs,
  ) -> HookRenderChunkReturn;

  async fn call_augment_chunk_hash(
    &self,
    _ctx: &SharedPluginContext,
    _chunk: &RollupRenderedChunk,
  ) -> HookAugmentChunkHashReturn;

  async fn call_render_error(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderErrorArgs,
  ) -> HookNoopReturn;

  async fn call_generate_bundle(
    &self,
    _ctx: &SharedPluginContext,
    _bundle: Vec<Output>,
    _is_write: bool,
  ) -> HookGenerateBundleReturn;

  async fn call_write_bundle(
    &self,
    _ctx: &SharedPluginContext,
    _bundle: Vec<Output>,
  ) -> HookWriteBundleReturn;
}

#[async_trait::async_trait]
impl<T: Plugin> Pluginable for T {
  fn call_name(&self) -> Cow<'static, str> {
    Plugin::name(self)
  }

  async fn call_build_start(&self, ctx: &SharedPluginContext) -> HookNoopReturn {
    Plugin::build_start(self, ctx).await
  }

  async fn call_resolve_id(
    &self,
    ctx: &SharedPluginContext,
    args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    Plugin::resolve_id(self, ctx, args).await
  }

  #[allow(deprecated)]
  async fn call_resolve_dynamic_import(
    &self,
    ctx: &SharedPluginContext,
    args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    Plugin::resolve_dynamic_import(self, ctx, args).await
  }

  async fn call_load(&self, ctx: &SharedPluginContext, args: &HookLoadArgs) -> HookLoadReturn {
    Plugin::load(self, ctx, args).await
  }

  async fn call_transform(
    &self,
    ctx: &TransformPluginContext<'_>,
    args: &HookTransformArgs,
  ) -> HookTransformReturn {
    Plugin::transform(self, ctx, args).await
  }

  async fn call_module_parsed(
    &self,
    ctx: &SharedPluginContext,
    module_info: Arc<ModuleInfo>,
  ) -> HookNoopReturn {
    Plugin::module_parsed(self, ctx, module_info).await
  }

  async fn call_build_end(
    &self,
    ctx: &SharedPluginContext,
    args: Option<&HookBuildEndArgs>,
  ) -> HookNoopReturn {
    Plugin::build_end(self, ctx, args).await
  }

  async fn call_render_start(&self, ctx: &SharedPluginContext) -> HookNoopReturn {
    Plugin::render_start(self, ctx).await
  }

  async fn call_banner(
    &self,
    ctx: &SharedPluginContext,
    args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn {
    Plugin::banner(self, ctx, args).await
  }

  async fn call_footer(
    &self,
    ctx: &SharedPluginContext,
    args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn {
    Plugin::footer(self, ctx, args).await
  }

  async fn call_intro(
    &self,
    ctx: &SharedPluginContext,
    args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn {
    Plugin::intro(self, ctx, args).await
  }

  async fn call_outro(
    &self,
    ctx: &SharedPluginContext,
    args: &HookInjectionArgs,
  ) -> HookInjectionOutputReturn {
    Plugin::outro(self, ctx, args).await
  }

  async fn call_render_chunk(
    &self,
    ctx: &SharedPluginContext,
    args: &HookRenderChunkArgs,
  ) -> HookRenderChunkReturn {
    Plugin::render_chunk(self, ctx, args).await
  }

  async fn call_augment_chunk_hash(
    &self,
    ctx: &SharedPluginContext,
    chunk: &RollupRenderedChunk,
  ) -> HookAugmentChunkHashReturn {
    Plugin::augment_chunk_hash(self, ctx, chunk).await
  }

  async fn call_render_error(
    &self,
    ctx: &SharedPluginContext,
    args: &HookRenderErrorArgs,
  ) -> HookNoopReturn {
    Plugin::render_error(self, ctx, args).await
  }

  async fn call_generate_bundle(
    &self,
    ctx: &SharedPluginContext,
    bundle: Vec<Output>,
    is_write: bool,
  ) -> HookGenerateBundleReturn {
    Plugin::generate_bundle(self, ctx, bundle, is_write).await
  }

  async fn call_write_bundle(
    &self,
    ctx: &SharedPluginContext,
    bundle: Vec<Output>,
  ) -> HookWriteBundleReturn {
    Plugin::write_bundle(self, ctx, bundle).await
  }

  fn call_transform_ast(
    &self,
    ctx: &SharedPluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    Plugin::transform_ast(self, ctx, args)
  }
}
