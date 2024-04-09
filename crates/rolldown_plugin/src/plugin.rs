use std::{any::Any, borrow::Cow, fmt::Debug};

use super::plugin_context::SharedPluginContext;
use crate::{
  worker_manager::WorkerManager, HookBuildEndArgs, HookLoadArgs, HookLoadOutput,
  HookRenderChunkArgs, HookRenderChunkOutput, HookResolveIdArgs, HookResolveIdOutput,
  HookTransformArgs,
};
use futures::future::{self, BoxFuture};
use rolldown_common::Output;
use rolldown_error::BuildError;

pub type HookResolveIdReturn = Result<Option<HookResolveIdOutput>, BuildError>;
pub type HookTransformReturn = Result<Option<HookLoadOutput>, BuildError>;
pub type HookLoadReturn = Result<Option<HookLoadOutput>, BuildError>;
pub type HookNoopReturn = Result<(), BuildError>;
pub type HookRenderChunkReturn = Result<Option<HookRenderChunkOutput>, BuildError>;

#[async_trait::async_trait]
pub trait Plugin: Any + Debug + Send + Sync + 'static {
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

  async fn load(&self, _ctx: &SharedPluginContext, _args: &HookLoadArgs) -> HookLoadReturn {
    Ok(None)
  }

  async fn transform(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookTransformArgs,
  ) -> HookTransformReturn {
    Ok(None)
  }

  async fn build_end(
    &self,
    _ctx: &SharedPluginContext,
    _args: Option<&HookBuildEndArgs>,
  ) -> HookNoopReturn {
    Ok(())
  }

  async fn render_chunk(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderChunkArgs,
  ) -> HookRenderChunkReturn {
    Ok(None)
  }

  // --- Generate hooks ---

  #[allow(clippy::ptr_arg)]
  async fn generate_bundle(
    &self,
    _ctx: &SharedPluginContext,
    _bundle: &Vec<Output>,
    _is_write: bool,
  ) -> HookNoopReturn {
    Ok(())
  }

  #[allow(clippy::ptr_arg)]
  async fn write_bundle(
    &self,
    _ctx: &SharedPluginContext,
    _bundle: &Vec<Output>,
  ) -> HookNoopReturn {
    Ok(())
  }
}

pub type BoxPlugin = Box<dyn Plugin>;

#[derive(Debug)]
pub enum PluginOrThreadSafePlugin {
  SinglePlugin(BoxPlugin),
  ThreadSafePlugin(Box<[BoxPlugin]>),
}

impl PluginOrThreadSafePlugin {
  pub(crate) async fn run_single<'a, R, F: FnOnce(&'a BoxPlugin) -> BoxFuture<R>>(
    &'a self,
    worker_manager: Option<&WorkerManager>,
    f: F,
  ) -> R {
    match self {
      Self::SinglePlugin(plugin) => f(plugin).await,
      Self::ThreadSafePlugin(plugins) => {
        let permit = worker_manager.unwrap().acquire().await;
        let plugin = &plugins[permit.worker_index() as usize];
        f(plugin).await
      }
    }
  }

  pub(crate) async fn run_all<'a, R, F: FnMut(&'a BoxPlugin) -> BoxFuture<R>>(
    &'a self,
    worker_manager: Option<&WorkerManager>,
    mut f: F,
  ) -> Vec<R> {
    match self {
      Self::SinglePlugin(plugin) => vec![f(plugin).await],
      Self::ThreadSafePlugin(plugins) => {
        let _permit = worker_manager.unwrap().acquire_all().await;
        future::join_all(plugins.iter().map(f)).await
      }
    }
  }
}

impl From<BoxPlugin> for PluginOrThreadSafePlugin {
  fn from(value: BoxPlugin) -> Self {
    Self::SinglePlugin(value)
  }
}

impl From<Box<[BoxPlugin]>> for PluginOrThreadSafePlugin {
  fn from(value: Box<[BoxPlugin]>) -> Self {
    Self::ThreadSafePlugin(value)
  }
}
