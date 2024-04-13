use std::{borrow::Cow, sync::Arc};

use futures::future::{self, BoxFuture};
use rolldown_plugin::Plugin;

use crate::worker_manager::WorkerManager;

use super::{BindingPluginOptions, JsPlugin};

#[derive(Debug)]
pub struct ParallelJsPlugin {
  plugins: Box<[JsPlugin]>,
  worker_manager: Arc<WorkerManager>,
}

impl ParallelJsPlugin {
  pub fn new_boxed(
    plugins: Vec<BindingPluginOptions>,
    worker_manager: Arc<WorkerManager>,
  ) -> Box<dyn Plugin> {
    let plugins = plugins.into_iter().map(JsPlugin::new_raw).collect::<Vec<_>>().into_boxed_slice();
    Box::new(Self { plugins, worker_manager })
  }

  async fn run_single<'a, R, F: FnOnce(&'a JsPlugin) -> BoxFuture<R>>(&'a self, f: F) -> R {
    let permit = self.worker_manager.acquire().await;
    let plugin = &self.plugins[permit.worker_index() as usize];
    f(plugin).await
  }

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

#[async_trait::async_trait]
impl Plugin for ParallelJsPlugin {
  fn name(&self) -> Cow<'static, str> {
    self.plugins[0].name()
  }

  // --- Build hooks ---

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    self.run_all(|plugin| plugin.build_start(ctx)).await?;
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookResolveIdArgs,
  ) -> rolldown_plugin::HookResolveIdReturn {
    self.run_single(|plugin| plugin.resolve_id(ctx, args)).await
  }

  async fn load(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookLoadArgs,
  ) -> rolldown_plugin::HookLoadReturn {
    self.run_single(|plugin| plugin.load(ctx, args)).await
  }

  async fn transform(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookTransformArgs,
  ) -> rolldown_plugin::HookTransformReturn {
    self.run_single(|plugin| plugin.transform(ctx, args)).await
  }

  async fn build_end(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.run_all(|plugin| plugin.build_end(ctx, args)).await?;
    Ok(())
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    self.run_single(|plugin| plugin.render_chunk(ctx, args)).await
  }

  // --- Output hooks ---

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &Vec<rolldown_common::Output>,
    is_write: bool,
  ) -> rolldown_plugin::HookNoopReturn {
    self.run_all(|plugin| plugin.generate_bundle(ctx, bundle, is_write)).await?;
    Ok(())
  }

  async fn write_bundle(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &Vec<rolldown_common::Output>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.run_all(|plugin| plugin.write_bundle(ctx, bundle)).await?;
    Ok(())
  }
}
