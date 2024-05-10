#[cfg(not(target_family = "wasm"))]
use std::borrow::Cow;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use futures::future::{self, BoxFuture};
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
  ) -> Box<dyn Plugin> {
    let plugins = plugins.into_iter().map(JsPlugin::new).collect::<Vec<_>>().into_boxed_slice();
    Box::new(Self { plugins, worker_manager })
  }

  fn first_plugin(&self) -> &JsPlugin {
    &self.plugins[0]
  }

  #[cfg(not(target_family = "wasm"))]
  async fn run_single<'a, R, F: FnOnce(&'a JsPlugin) -> BoxFuture<R>>(&'a self, f: F) -> R {
    let permit = self.worker_manager.acquire().await;
    let plugin = &self.plugins[permit.worker_index() as usize];
    f(plugin).await
  }

  #[cfg(not(target_family = "wasm"))]
  async fn run_all<'a, R, E: std::fmt::Debug, F: FnMut(&'a JsPlugin) -> BoxFuture<Result<R, E>>>(
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
#[async_trait::async_trait]
impl Plugin for ParallelJsPlugin {
  fn name(&self) -> Cow<'static, str> {
    self.first_plugin().name()
  }

  // --- Build hooks ---

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().build_start.is_some() {
      self.run_all(|plugin| plugin.build_start(ctx)).await?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookResolveIdArgs,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if self.first_plugin().resolve_id.is_some() {
      self.run_single(|plugin| plugin.resolve_id(ctx, args)).await
    } else {
      Ok(None)
    }
  }

  async fn load(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookLoadArgs,
  ) -> rolldown_plugin::HookLoadReturn {
    if self.first_plugin().load.is_some() {
      self.run_single(|plugin| plugin.load(ctx, args)).await
    } else {
      Ok(None)
    }
  }

  async fn transform(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookTransformArgs,
  ) -> rolldown_plugin::HookTransformReturn {
    if self.first_plugin().transform.is_some() {
      self.run_single(|plugin| plugin.transform(ctx, args)).await
    } else {
      Ok(None)
    }
  }

  async fn build_end(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().build_end.is_some() {
      self.run_all(|plugin| plugin.build_end(ctx, args)).await?;
    }
    Ok(())
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if self.first_plugin().render_chunk.is_some() {
      self.run_single(|plugin| plugin.render_chunk(ctx, args)).await
    } else {
      Ok(None)
    }
  }

  // --- Output hooks ---

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &mut Vec<rolldown_common::Output>,
    is_write: bool,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().generate_bundle.is_some() {
      self.run_single(|plugin| plugin.generate_bundle(ctx, bundle, is_write)).await
    } else {
      Ok(())
    }
  }

  async fn write_bundle(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &mut Vec<rolldown_common::Output>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().write_bundle.is_some() {
      self.run_single(|plugin| plugin.write_bundle(ctx, bundle)).await
    } else {
      Ok(())
    }
  }
}
