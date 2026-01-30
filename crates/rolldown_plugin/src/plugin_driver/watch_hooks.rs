use crate::HookNoopReturn;
use crate::PluginDriver;
use anyhow::Context as _;
use rolldown_common::WatcherChangeKind;
use rolldown_error::CausedPlugin;

impl PluginDriver {
  pub async fn watch_change(&self, path: &str, event: WatcherChangeKind) -> HookNoopReturn {
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_watch_change_meta)
    {
      let start = self.start_timing();
      let result = plugin.call_watch_change(ctx, path, event).await;
      self.record_timing(plugin_idx, start);
      result.with_context(|| CausedPlugin::new(plugin.call_name()))?;
    }
    Ok(())
  }

  pub async fn close_watcher(&self) -> HookNoopReturn {
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_close_watcher_meta)
    {
      let start = self.start_timing();
      let result = plugin.call_close_watcher(ctx).await;
      self.record_timing(plugin_idx, start);
      result.with_context(|| CausedPlugin::new(plugin.call_name()))?;
    }
    Ok(())
  }
}
