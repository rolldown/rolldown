use rolldown_common::WatcherChangeKind;
use rolldown_error::CausedPlugin;

use crate::{HookNoopReturn, PluginDriver};

impl PluginDriver {
  pub async fn watch_change(&self, path: &str, event: WatcherChangeKind) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_watch_change_meta)
    {
      plugin
        .call_watch_change(ctx, path, event)
        .await
        .map_err(|err| err.with_caused_plugin(CausedPlugin::new(plugin.call_name())))?;
    }
    Ok(())
  }

  pub async fn close_watcher(&self) -> HookNoopReturn {
    for (_, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_close_watcher_meta)
    {
      plugin
        .call_close_watcher(ctx)
        .await
        .map_err(|err| err.with_caused_plugin(CausedPlugin::new(plugin.call_name())))?;
    }
    Ok(())
  }
}
