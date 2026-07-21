use std::{any::Any, borrow::Cow, fmt::Debug, future::Future, pin::Pin, sync::Arc};

use super::plugin_context::PluginContext;
use crate::{
  HookAddonArgs, HookBuildEndArgs, HookBuildStartArgs, HookCloseBundleArgs, HookGenerateBundleArgs,
  HookInjectionOutputReturn, HookLoadArgs, HookRenderChunkArgs, HookRenderStartArgs,
  HookResolveFileUrlArgs, HookResolveIdArgs, HookTransformArgs, HookUsage, Plugin, PluginHookMeta,
  SharedLoadPluginContext, SharedTransformPluginContext,
  types::{
    hook_render_error::HookRenderErrorArgs, hook_transform_ast_args::HookTransformAstArgs,
    hook_write_bundle_args::HookWriteBundleArgs,
  },
};
use anyhow::Ok;
use rolldown_common::{ModuleInfo, NormalModule, RollupRenderedChunk, WatcherChangeKind};

pub use crate::plugin::HookAugmentChunkHashReturn;
pub use crate::plugin::HookLoadReturn;
pub use crate::plugin::HookNoopReturn;
pub use crate::plugin::HookRenderChunkReturn;
pub use crate::plugin::HookResolveFileUrlReturn;
pub use crate::plugin::HookResolveIdReturn;
pub use crate::plugin::HookTransformAstReturn;
pub use crate::plugin::HookTransformReturn;

pub type SharedPluginable = Arc<dyn Pluginable>;
type HookFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// `Pluginable` is under the hood trait that rolldown to run. It's not recommended to use this trait directly.
/// To create a plugin, you should use [Plugin] trait instead.
///
/// The main reason we don't expose this trait is that its boxed futures make it harder for
/// rust-analyzer to provide a good auto-completion experience.
pub trait Pluginable: Any + Debug + Send + Sync + 'static {
  fn call_name(&self) -> Cow<'static, str>;

  // The `option` hook consider call at node side.

  // --- Build hooks ---

  fn call_build_start<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookBuildStartArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn>;

  fn call_build_start_meta(&self) -> Option<PluginHookMeta>;

  fn call_resolve_id<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookResolveIdArgs<'a>,
  ) -> HookFuture<'a, HookResolveIdReturn>;

  fn call_resolve_id_meta(&self) -> Option<PluginHookMeta>;

  #[deprecated(
    note = "This hook is only for rollup compatibility, please use `resolve_id` instead."
  )]
  fn call_resolve_dynamic_import<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookResolveIdArgs<'a>,
  ) -> HookFuture<'a, HookResolveIdReturn>;

  fn call_resolve_dynamic_import_meta(&self) -> Option<PluginHookMeta>;

  fn call_load<'a>(
    &'a self,
    _ctx: SharedLoadPluginContext,
    _args: &'a HookLoadArgs<'a>,
  ) -> HookFuture<'a, HookLoadReturn>;

  fn call_load_meta(&self) -> Option<PluginHookMeta>;

  fn call_transform<'a>(
    &'a self,
    _ctx: SharedTransformPluginContext,
    _args: &'a HookTransformArgs<'a>,
  ) -> HookFuture<'a, HookTransformReturn>;

  fn call_transform_meta(&self) -> Option<PluginHookMeta>;

  fn call_transform_ast<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    args: HookTransformAstArgs<'a>,
  ) -> HookFuture<'a, HookTransformAstReturn>;

  fn call_transform_ast_meta(&self) -> Option<PluginHookMeta>;

  fn call_module_parsed<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _module_info: Arc<ModuleInfo>,
    _normal_module: &'a NormalModule,
  ) -> HookFuture<'a, HookNoopReturn>;

  fn call_module_parsed_meta(&self) -> Option<PluginHookMeta>;

  fn call_build_end<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: Option<&'a HookBuildEndArgs<'a>>,
  ) -> HookFuture<'a, HookNoopReturn>;

  fn call_build_end_meta(&self) -> Option<PluginHookMeta>;

  // --- Generate hooks ---

  fn call_render_start<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookRenderStartArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn>;

  fn call_render_start_meta(&self) -> Option<PluginHookMeta>;

  fn call_banner<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookAddonArgs,
  ) -> HookFuture<'a, HookInjectionOutputReturn>;

  fn call_banner_meta(&self) -> Option<PluginHookMeta>;

  fn call_footer<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookAddonArgs,
  ) -> HookFuture<'a, HookInjectionOutputReturn>;

  fn call_footer_meta(&self) -> Option<PluginHookMeta>;

  fn call_intro<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookAddonArgs,
  ) -> HookFuture<'a, HookInjectionOutputReturn>;

  fn call_intro_meta(&self) -> Option<PluginHookMeta>;

  fn call_outro<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookAddonArgs,
  ) -> HookFuture<'a, HookInjectionOutputReturn>;

  fn call_outro_meta(&self) -> Option<PluginHookMeta>;

  fn call_render_chunk<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookRenderChunkArgs<'a>,
  ) -> HookFuture<'a, HookRenderChunkReturn>;

  fn call_render_chunk_meta(&self) -> Option<PluginHookMeta>;

  fn call_augment_chunk_hash<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _chunk: Arc<RollupRenderedChunk>,
  ) -> HookFuture<'a, HookAugmentChunkHashReturn>;

  fn call_augment_chunk_hash_meta(&self) -> Option<PluginHookMeta>;

  fn call_resolve_file_url<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookResolveFileUrlArgs<'a>,
  ) -> HookFuture<'a, HookResolveFileUrlReturn>;

  fn call_resolve_file_url_meta(&self) -> Option<PluginHookMeta>;

  fn call_render_error<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a HookRenderErrorArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn>;

  fn call_render_error_meta(&self) -> Option<PluginHookMeta>;

  fn call_generate_bundle<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a mut HookGenerateBundleArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn>;

  fn call_generate_bundle_meta(&self) -> Option<PluginHookMeta>;

  fn call_write_bundle<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: &'a mut HookWriteBundleArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn>;

  fn call_write_bundle_meta(&self) -> Option<PluginHookMeta>;

  fn call_close_bundle<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _args: Option<&'a HookCloseBundleArgs<'a>>,
  ) -> HookFuture<'a, HookNoopReturn>;

  fn call_close_bundle_meta(&self) -> Option<PluginHookMeta>;

  fn call_watch_change<'a>(
    &'a self,
    _ctx: &'a PluginContext,
    _path: &'a str,
    _event: WatcherChangeKind,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(async { Ok(()) })
  }

  fn call_watch_change_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn call_close_watcher<'a>(&'a self, _ctx: &'a PluginContext) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(async { Ok(()) })
  }

  fn call_close_watcher_meta(&self) -> Option<PluginHookMeta> {
    None
  }

  fn call_hook_usage(&self) -> HookUsage;
}

impl<T: Plugin> Pluginable for T {
  fn call_name(&self) -> Cow<'static, str> {
    Plugin::name(self)
  }

  fn call_build_start<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookBuildStartArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::build_start(self, ctx, args))
  }

  fn call_build_start_meta(&self) -> Option<PluginHookMeta> {
    Plugin::build_start_meta(self)
  }

  fn call_resolve_id<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookResolveIdArgs<'a>,
  ) -> HookFuture<'a, HookResolveIdReturn> {
    Box::pin(Plugin::resolve_id(self, ctx, args))
  }

  fn call_resolve_id_meta(&self) -> Option<PluginHookMeta> {
    Plugin::resolve_id_meta(self)
  }

  #[expect(deprecated)]
  fn call_resolve_dynamic_import<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookResolveIdArgs<'a>,
  ) -> HookFuture<'a, HookResolveIdReturn> {
    Box::pin(Plugin::resolve_dynamic_import(self, ctx, args))
  }

  fn call_resolve_dynamic_import_meta(&self) -> Option<PluginHookMeta> {
    Plugin::resolve_dynamic_import_meta(self)
  }

  fn call_load<'a>(
    &'a self,
    ctx: SharedLoadPluginContext,
    args: &'a HookLoadArgs<'a>,
  ) -> HookFuture<'a, HookLoadReturn> {
    Box::pin(Plugin::load(self, ctx, args))
  }

  fn call_load_meta(&self) -> Option<PluginHookMeta> {
    Plugin::load_meta(self)
  }

  fn call_transform<'a>(
    &'a self,
    ctx: SharedTransformPluginContext,
    args: &'a HookTransformArgs<'a>,
  ) -> HookFuture<'a, HookTransformReturn> {
    Box::pin(Plugin::transform(self, ctx, args))
  }

  fn call_transform_meta(&self) -> Option<PluginHookMeta> {
    Plugin::transform_meta(self)
  }

  fn call_module_parsed<'a>(
    &'a self,
    ctx: &'a PluginContext,
    module_info: Arc<ModuleInfo>,
    normal_module: &'a NormalModule,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::module_parsed(self, ctx, module_info, normal_module))
  }

  fn call_module_parsed_meta(&self) -> Option<PluginHookMeta> {
    Plugin::module_parsed_meta(self)
  }

  fn call_build_end<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: Option<&'a HookBuildEndArgs<'a>>,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::build_end(self, ctx, args))
  }

  fn call_build_end_meta(&self) -> Option<PluginHookMeta> {
    Plugin::build_end_meta(self)
  }

  fn call_render_start<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookRenderStartArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::render_start(self, ctx, args))
  }

  fn call_render_start_meta(&self) -> Option<PluginHookMeta> {
    Plugin::render_start_meta(self)
  }

  fn call_banner<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookAddonArgs,
  ) -> HookFuture<'a, HookInjectionOutputReturn> {
    Box::pin(Plugin::banner(self, ctx, args))
  }

  fn call_banner_meta(&self) -> Option<PluginHookMeta> {
    Plugin::banner_meta(self)
  }

  fn call_footer<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookAddonArgs,
  ) -> HookFuture<'a, HookInjectionOutputReturn> {
    Box::pin(Plugin::footer(self, ctx, args))
  }

  fn call_footer_meta(&self) -> Option<PluginHookMeta> {
    Plugin::footer_meta(self)
  }

  fn call_intro<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookAddonArgs,
  ) -> HookFuture<'a, HookInjectionOutputReturn> {
    Box::pin(Plugin::intro(self, ctx, args))
  }

  fn call_intro_meta(&self) -> Option<PluginHookMeta> {
    Plugin::intro_meta(self)
  }

  fn call_outro<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookAddonArgs,
  ) -> HookFuture<'a, HookInjectionOutputReturn> {
    Box::pin(Plugin::outro(self, ctx, args))
  }

  fn call_outro_meta(&self) -> Option<PluginHookMeta> {
    Plugin::outro_meta(self)
  }

  fn call_render_chunk<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookRenderChunkArgs<'a>,
  ) -> HookFuture<'a, HookRenderChunkReturn> {
    Box::pin(Plugin::render_chunk(self, ctx, args))
  }

  fn call_render_chunk_meta(&self) -> Option<PluginHookMeta> {
    Plugin::render_chunk_meta(self)
  }

  fn call_augment_chunk_hash<'a>(
    &'a self,
    ctx: &'a PluginContext,
    chunk: Arc<RollupRenderedChunk>,
  ) -> HookFuture<'a, HookAugmentChunkHashReturn> {
    Box::pin(Plugin::augment_chunk_hash(self, ctx, chunk))
  }

  fn call_augment_chunk_hash_meta(&self) -> Option<PluginHookMeta> {
    Plugin::augment_chunk_hash_meta(self)
  }

  fn call_resolve_file_url<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookResolveFileUrlArgs<'a>,
  ) -> HookFuture<'a, HookResolveFileUrlReturn> {
    Box::pin(Plugin::resolve_file_url(self, ctx, args))
  }

  fn call_resolve_file_url_meta(&self) -> Option<PluginHookMeta> {
    Plugin::resolve_file_url_meta(self)
  }

  fn call_render_error<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a HookRenderErrorArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::render_error(self, ctx, args))
  }

  fn call_render_error_meta(&self) -> Option<PluginHookMeta> {
    Plugin::render_error_meta(self)
  }

  fn call_generate_bundle<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a mut HookGenerateBundleArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::generate_bundle(self, ctx, args))
  }

  fn call_generate_bundle_meta(&self) -> Option<PluginHookMeta> {
    Plugin::generate_bundle_meta(self)
  }

  fn call_write_bundle<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: &'a mut HookWriteBundleArgs<'a>,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::write_bundle(self, ctx, args))
  }

  fn call_write_bundle_meta(&self) -> Option<PluginHookMeta> {
    Plugin::write_bundle_meta(self)
  }

  fn call_close_bundle<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: Option<&'a HookCloseBundleArgs<'a>>,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::close_bundle(self, ctx, args))
  }

  fn call_close_bundle_meta(&self) -> Option<PluginHookMeta> {
    Plugin::close_bundle_meta(self)
  }

  fn call_watch_change<'a>(
    &'a self,
    ctx: &'a PluginContext,
    path: &'a str,
    event: WatcherChangeKind,
  ) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::watch_change(self, ctx, path, event))
  }

  fn call_watch_change_meta(&self) -> Option<PluginHookMeta> {
    Plugin::watch_change_meta(self)
  }

  fn call_close_watcher<'a>(&'a self, ctx: &'a PluginContext) -> HookFuture<'a, HookNoopReturn> {
    Box::pin(Plugin::close_watcher(self, ctx))
  }

  fn call_close_watcher_meta(&self) -> Option<PluginHookMeta> {
    Plugin::close_watcher_meta(self)
  }

  fn call_transform_ast<'a>(
    &'a self,
    ctx: &'a PluginContext,
    args: HookTransformAstArgs<'a>,
  ) -> HookFuture<'a, HookTransformAstReturn> {
    Box::pin(Plugin::transform_ast(self, ctx, args))
  }

  fn call_transform_ast_meta(&self) -> Option<PluginHookMeta> {
    Plugin::transform_ast_meta(self)
  }

  fn call_hook_usage(&self) -> HookUsage {
    Plugin::register_hook_usage(self)
  }
}
