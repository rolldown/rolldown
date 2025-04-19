use crate::HookNoopReturn;
use crate::HookUsage;
use crate::PluginDriver;
use rolldown_common::WatcherChangeKind;

impl PluginDriver {
  pub async fn watch_change(&self, path: &str, event: WatcherChangeKind) -> HookNoopReturn {
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_watch_change_meta)
    {
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::WatchChange) {
        continue;
      }
      plugin.call_watch_change(ctx, path, event).await?;
    }
    Ok(())
  }

  pub async fn close_watcher(&self) -> HookNoopReturn {
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_close_watcher_meta)
    {
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::CloseWatcher) {
        continue;
      }
      plugin.call_close_watcher(ctx).await?;
    }
    Ok(())
  }
}
