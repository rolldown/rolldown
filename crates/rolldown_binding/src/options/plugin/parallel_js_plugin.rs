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
  // Shared worker manager for coordinating JavaScript plugin execution across threads
  worker_manager: Arc<WorkerManager>,
}

#[cfg(not(target_family = "wasm"))]
impl ParallelJsPlugin {
  pub fn new_boxed(
    plugins: Vec<BindingPluginOptions>,
    // Accept shared worker manager for JavaScript plugin coordination
    worker_manager: Arc<WorkerManager>,
  ) -> Box<dyn Pluginable> {
    let plugins = plugins.into_iter().map(JsPlugin::new).collect::<Vec<_>>().into_boxed_slice();
    Box::new(Self { plugins, worker_manager })
  }

  pub fn new_shared(
    plugins: Vec<BindingPluginOptions>,
    // Accept shared worker manager for JavaScript plugin coordination
    worker_manager: Arc<WorkerManager>,
  ) -> Arc<dyn Pluginable> {
    let plugins = plugins.into_iter().map(JsPlugin::new).collect::<Vec<_>>().into_boxed_slice();
    // Return shared parallel plugin for cross-thread usage
    Arc::new(Self { plugins, worker_manager })
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
      self.run_all(|plugin| plugin.call_build_start(ctx, args)).await?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if self.first_plugin().resolve_id.is_some() {
      self.run_single(|plugin| plugin.call_resolve_id(ctx, args)).await
    } else {
      Ok(None)
    }
  }

  async fn load(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if self.first_plugin().load.is_some() {
      self.run_single(|plugin| plugin.call_load(ctx, args)).await
    } else {
      Ok(None)
    }
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if self.first_plugin().transform.is_some() {
      self.run_single(|plugin| plugin.call_transform(ctx, args)).await
    } else {
      Ok(None)
    }
  }

  async fn build_end(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().build_end.is_some() {
      self.run_all(|plugin| plugin.call_build_end(ctx, args)).await?;
    }
    Ok(())
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if self.first_plugin().render_chunk.is_some() {
      self.run_single(|plugin| plugin.call_render_chunk(ctx, args)).await
    } else {
      Ok(None)
    }
  }

  // --- Output hooks ---

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().generate_bundle.is_some() {
      self.run_single(|plugin| plugin.call_generate_bundle(ctx, args)).await
    } else {
      Ok(())
    }
  }

  async fn write_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookWriteBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().write_bundle.is_some() {
      self.run_single(|plugin| plugin.call_write_bundle(ctx, args)).await
    } else {
      Ok(())
    }
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::all()
  }
}
