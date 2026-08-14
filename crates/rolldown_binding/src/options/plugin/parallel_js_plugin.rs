#![cfg(not(target_family = "wasm"))]

#[cfg(not(target_family = "wasm"))]
use std::borrow::Cow;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use futures::future::{self, BoxFuture};
#[cfg(not(target_family = "wasm"))]
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin::HookUsage;
#[cfg(not(target_family = "wasm"))]
use rolldown_plugin::Plugin;

use crate::worker_manager::WorkerManager;

#[cfg(not(target_family = "wasm"))]
use super::BindingPluginOptions;
use super::JsPlugin;

#[derive(Debug)]
#[cfg_attr(target_family = "wasm", allow(unused))]
pub struct ParallelJsPlugin {
  plugins: Box<[JsPlugin]>,
  worker_manager: Arc<WorkerManager>,
}

#[cfg(not(target_family = "wasm"))]
impl ParallelJsPlugin {
  pub fn new_boxed(
    plugins: Vec<BindingPluginOptions>,
    worker_manager: Arc<WorkerManager>,
  ) -> napi::Result<Box<dyn Pluginable>> {
    let plugins =
      plugins.into_iter().map(JsPlugin::new).collect::<napi::Result<Vec<_>>>()?.into_boxed_slice();
    Ok(Box::new(Self { plugins, worker_manager }))
  }

  pub fn new_shared(
    plugins: Vec<BindingPluginOptions>,
    worker_manager: Arc<WorkerManager>,
  ) -> napi::Result<Arc<dyn Pluginable>> {
    let plugins =
      plugins.into_iter().map(JsPlugin::new).collect::<napi::Result<Vec<_>>>()?.into_boxed_slice();
    Ok(Arc::new(Self { plugins, worker_manager }))
  }

  fn first_plugin(&self) -> &JsPlugin {
    &self.plugins[0]
  }

  #[cfg(not(target_family = "wasm"))]
  async fn run_single<'a, R, F: FnOnce(&'a JsPlugin) -> BoxFuture<'a, R>>(&'a self, f: F) -> R {
    let permit = self.worker_manager.acquire().await;
    let plugin = &self.plugins[permit.worker_index() as usize];
    f(plugin).await
  }

  #[cfg(not(target_family = "wasm"))]
  async fn run_all<
    'a,
    R,
    E: std::fmt::Debug,
    F: FnMut(&'a JsPlugin) -> BoxFuture<'a, Result<R, E>>,
  >(
    &'a self,
    f: F,
  ) -> Result<Vec<R>, E> {
    let _permit = self.worker_manager.acquire_all().await;
    let results = future::join_all(self.plugins.iter().map(f)).await;
    let mut ok_list: Vec<R> = Vec::with_capacity(results.len());
    for result in results {
      ok_list.push(result?);
    }
    Ok(ok_list)
  }
}

#[cfg(not(target_family = "wasm"))]
impl Plugin for ParallelJsPlugin {
  fn name(&self) -> Cow<'static, str> {
    self.first_plugin().call_name()
  }

  // --- Build hooks ---

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().build_start.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::build_start(plugin, ctx, args))).await?;
    }
    Ok(())
  }

  fn build_start_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::build_start_meta(self.first_plugin())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if self.first_plugin().resolve_id.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::resolve_id(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn resolve_id_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::resolve_id_meta(self.first_plugin())
  }

  #[expect(deprecated)]
  async fn resolve_dynamic_import(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if self.first_plugin().resolve_dynamic_import.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::resolve_dynamic_import(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn resolve_dynamic_import_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::resolve_dynamic_import_meta(self.first_plugin())
  }

  async fn load(
    &self,
    ctx: rolldown_plugin::SharedLoadPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if self.first_plugin().load.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::load(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn load_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::load_meta(self.first_plugin())
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if self.first_plugin().transform.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::transform(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn transform_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::transform_meta(self.first_plugin())
  }

  async fn module_parsed(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    module_info: Arc<rolldown_common::ModuleInfo>,
    normal_module: &rolldown_common::NormalModule,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().module_parsed.is_some() {
      self
        .run_all(|plugin| {
          Box::pin(Plugin::module_parsed(plugin, ctx, Arc::clone(&module_info), normal_module))
        })
        .await?;
    }
    Ok(())
  }

  fn module_parsed_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::module_parsed_meta(self.first_plugin())
  }

  async fn build_end(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().build_end.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::build_end(plugin, ctx, args))).await?;
    }
    Ok(())
  }

  fn build_end_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::build_end_meta(self.first_plugin())
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if self.first_plugin().render_chunk.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::render_chunk(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn render_chunk_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::render_chunk_meta(self.first_plugin())
  }

  // --- Output hooks ---

  async fn render_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().render_start.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::render_start(plugin, ctx, args))).await?;
    }
    Ok(())
  }

  fn render_start_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::render_start_meta(self.first_plugin())
  }

  async fn banner(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if self.first_plugin().banner.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::banner(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn banner_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::banner_meta(self.first_plugin())
  }

  async fn intro(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if self.first_plugin().intro.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::intro(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn intro_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::intro_meta(self.first_plugin())
  }

  async fn outro(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if self.first_plugin().outro.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::outro(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn outro_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::outro_meta(self.first_plugin())
  }

  async fn footer(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    if self.first_plugin().footer.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::footer(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  fn footer_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::footer_meta(self.first_plugin())
  }

  async fn augment_chunk_hash(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    chunk: Arc<rolldown_common::RollupRenderedChunk>,
  ) -> rolldown_plugin::HookAugmentChunkHashReturn {
    if self.first_plugin().augment_chunk_hash.is_some() {
      self
        .run_single(|plugin| Box::pin(Plugin::augment_chunk_hash(plugin, ctx, Arc::clone(&chunk))))
        .await
    } else {
      Ok(None)
    }
  }

  fn augment_chunk_hash_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::augment_chunk_hash_meta(self.first_plugin())
  }

  async fn render_error(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderErrorArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().render_error.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::render_error(plugin, ctx, args))).await?;
    }
    Ok(())
  }

  fn render_error_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::render_error_meta(self.first_plugin())
  }

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().generate_bundle.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::generate_bundle(plugin, ctx, args))).await
    } else {
      Ok(())
    }
  }

  fn generate_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::generate_bundle_meta(self.first_plugin())
  }

  async fn write_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookWriteBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().write_bundle.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::write_bundle(plugin, ctx, args))).await
    } else {
      Ok(())
    }
  }

  fn write_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::write_bundle_meta(self.first_plugin())
  }

  async fn close_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: Option<&rolldown_plugin::HookCloseBundleArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().close_bundle.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::close_bundle(plugin, ctx, args))).await?;
    }
    Ok(())
  }

  fn close_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::close_bundle_meta(self.first_plugin())
  }

  async fn watch_change(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    path: &str,
    event: rolldown_common::WatcherChangeKind,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().watch_change.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::watch_change(plugin, ctx, path, event))).await?;
    }
    Ok(())
  }

  fn watch_change_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::watch_change_meta(self.first_plugin())
  }

  async fn close_watcher(
    &self,
    ctx: &rolldown_plugin::PluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().close_watcher.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::close_watcher(plugin, ctx))).await?;
    }
    Ok(())
  }

  fn close_watcher_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Plugin::close_watcher_meta(self.first_plugin())
  }

  fn register_hook_usage(&self) -> HookUsage {
    Plugin::register_hook_usage(self.first_plugin())
  }
}
