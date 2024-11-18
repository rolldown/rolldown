use std::{borrow::Cow, future::Future};

use rolldown_common::WatcherChangeKind;
use rolldown_plugin::{
  HookLoadArgs, HookLoadReturn, HookNoopReturn, HookResolveIdArgs, HookResolveIdReturn,
};

pub trait CallablePlugin: Send + Sync + 'static {
  fn name(&self) -> Cow<'static, str>;

  fn resolve_id(
    &self,
    args: &HookResolveIdArgs,
  ) -> impl Future<Output = HookResolveIdReturn> + Send;
  fn load(&self, args: &HookLoadArgs) -> impl Future<Output = HookLoadReturn> + Send;
  fn watch_change(
    &self,
    path: &str,
    event: WatcherChangeKind,
  ) -> impl std::future::Future<Output = HookNoopReturn> + Send;
}

#[async_trait::async_trait]
pub trait CallablePluginAsyncTrait: Send + Sync + 'static {
  fn name(&self) -> Cow<'static, str>;

  async fn resolve_id(&self, args: &HookResolveIdArgs) -> HookResolveIdReturn;
  async fn load(&self, args: &HookLoadArgs) -> HookLoadReturn;
  async fn watch_change(&self, path: &str, event: WatcherChangeKind) -> HookNoopReturn;
}

#[async_trait::async_trait]
impl<T: CallablePlugin> CallablePluginAsyncTrait for T {
  fn name(&self) -> Cow<'static, str> {
    CallablePlugin::name(self)
  }

  async fn resolve_id(&self, args: &HookResolveIdArgs) -> HookResolveIdReturn {
    CallablePlugin::resolve_id(self, args).await
  }

  async fn load(&self, args: &HookLoadArgs) -> HookLoadReturn {
    CallablePlugin::load(self, args).await
  }

  async fn watch_change(&self, path: &str, event: WatcherChangeKind) -> HookNoopReturn {
    CallablePlugin::watch_change(self, path, event).await
  }
}
