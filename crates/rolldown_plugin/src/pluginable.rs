use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use super::plugin_context::SharedPluginContext;
use crate::{
  transform_plugin_context::TransformPluginContext,
  types::{
    hook_footer_args::HookFooterArgs, hook_render_error::HookRenderErrorArgs,
    hook_transform_ast_args::HookTransformAstArgs,
  },
  HookBannerArgs, HookBuildEndArgs, HookLoadArgs, HookRenderChunkArgs,
  HookResolveDynamicImportArgs, HookResolveIdArgs, HookTransformArgs, Plugin,
};
use rolldown_common::{ModuleInfo, Output, RollupRenderedChunk};

pub use crate::plugin::HookAugmentChunkHashReturn;
pub use crate::plugin::HookBannerOutputReturn;
pub use crate::plugin::HookFooterOutputReturn;
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

  async fn banner(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookBannerArgs,
  ) -> HookBannerOutputReturn {
    Ok(None)
  }

  async fn footer(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookFooterArgs,
  ) -> HookFooterOutputReturn {
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

#[async_trait::async_trait]
impl<T: Plugin> Pluginable for T {
  fn name(&self) -> Cow<'static, str> {
    Plugin::name(self)
  }

  async fn build_start(&self, ctx: &SharedPluginContext) -> HookNoopReturn {
    Plugin::build_start(self, ctx).await
  }

  async fn resolve_id(
    &self,
    ctx: &SharedPluginContext,
    args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    Plugin::resolve_id(self, ctx, args).await
  }

  #[allow(deprecated)]
  async fn resolve_dynamic_import(
    &self,
    ctx: &SharedPluginContext,
    args: &HookResolveDynamicImportArgs,
  ) -> HookResolveIdReturn {
    Plugin::resolve_dynamic_import(self, ctx, args).await
  }

  async fn load(&self, ctx: &SharedPluginContext, args: &HookLoadArgs) -> HookLoadReturn {
    Plugin::load(self, ctx, args).await
  }

  async fn transform(
    &self,
    ctx: &TransformPluginContext<'_>,
    args: &HookTransformArgs,
  ) -> HookTransformReturn {
    Plugin::transform(self, ctx, args).await
  }

  async fn module_parsed(
    &self,
    ctx: &SharedPluginContext,
    module_info: Arc<ModuleInfo>,
  ) -> HookNoopReturn {
    Plugin::module_parsed(self, ctx, module_info).await
  }

  async fn build_end(
    &self,
    ctx: &SharedPluginContext,
    args: Option<&HookBuildEndArgs>,
  ) -> HookNoopReturn {
    Plugin::build_end(self, ctx, args).await
  }

  async fn render_start(&self, ctx: &SharedPluginContext) -> HookNoopReturn {
    Plugin::render_start(self, ctx).await
  }

  async fn banner(
    &self,
    ctx: &SharedPluginContext,
    args: &HookBannerArgs,
  ) -> HookBannerOutputReturn {
    Plugin::banner(self, ctx, args).await
  }

  async fn footer(
    &self,
    ctx: &SharedPluginContext,
    args: &HookFooterArgs,
  ) -> HookFooterOutputReturn {
    Plugin::footer(self, ctx, args).await
  }

  async fn render_chunk(
    &self,
    ctx: &SharedPluginContext,
    args: &HookRenderChunkArgs,
  ) -> HookRenderChunkReturn {
    Plugin::render_chunk(self, ctx, args).await
  }

  async fn augment_chunk_hash(
    &self,
    ctx: &SharedPluginContext,
    chunk: &RollupRenderedChunk,
  ) -> HookAugmentChunkHashReturn {
    Plugin::augment_chunk_hash(self, ctx, chunk).await
  }

  async fn render_error(
    &self,
    ctx: &SharedPluginContext,
    args: &HookRenderErrorArgs,
  ) -> HookNoopReturn {
    Plugin::render_error(self, ctx, args).await
  }

  async fn generate_bundle(
    &self,
    ctx: &SharedPluginContext,
    bundle: &mut Vec<Output>,
    is_write: bool,
  ) -> HookNoopReturn {
    Plugin::generate_bundle(self, ctx, bundle, is_write).await
  }

  async fn write_bundle(
    &self,
    ctx: &SharedPluginContext,
    bundle: &mut Vec<Output>,
  ) -> HookNoopReturn {
    Plugin::write_bundle(self, ctx, bundle).await
  }

  fn transform_ast(
    &self,
    ctx: &SharedPluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    Plugin::transform_ast(self, ctx, args)
  }
}
