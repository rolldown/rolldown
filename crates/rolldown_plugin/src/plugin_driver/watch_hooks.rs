use arcstr::ArcStr;

use crate::HookNoopReturn;
use crate::PluginDriver;
use crate::types::hook_hot_update_args::HookHotUpdateArgs;
use anyhow::Context;
use rolldown_common::WatcherChangeKind;
use rolldown_error::CausedPlugin;

impl PluginDriver {
  #[tracing::instrument(
    level = "trace",
    target = "rolldown_plugin::plugin_driver::watch_hooks::total::watch_change",
    skip(self)
  )]
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

  /// Dev-mode only (experimental). Runs the `hotUpdate` replace-chain over plugins in hook
  /// order: each plugin receives the current affected-module set for the changed file and may
  /// replace it (an empty vec suppresses the update). Returns the final set, or `None` when no
  /// plugin returned a replacement — the caller's default set stands, and the caller can tell
  /// an explicit plugin selection apart from the default flow.
  #[tracing::instrument(
    level = "trace",
    target = "rolldown_plugin::plugin_driver::watch_hooks::total::hot_update",
    skip(self, default_modules)
  )]
  pub async fn hot_update(
    &self,
    kind: WatcherChangeKind,
    file: &str,
    default_modules: Vec<ArcStr>,
  ) -> anyhow::Result<Option<Vec<ArcStr>>> {
    let mut args = HookHotUpdateArgs { kind, file: ArcStr::from(file), modules: default_modules };
    let mut replaced = false;
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_hot_update_meta)
    {
      let start = self.start_timing();
      let result = plugin.call_hot_update(ctx, &args).await;
      self.record_timing(plugin_idx, start);
      if let Some(modules) = result.with_context(|| CausedPlugin::new(plugin.call_name()))? {
        args.modules = modules;
        replaced = true;
      }
    }
    Ok(replaced.then_some(args.modules))
  }

  /// Cheap check for the HMR stage to skip the `hotUpdate` path entirely when no plugin
  /// registered the hook.
  pub fn has_hot_update_hook(&self) -> bool {
    !self.order_by_hot_update_meta.is_empty()
  }

  #[tracing::instrument(
    level = "trace",
    target = "rolldown_plugin::plugin_driver::watch_hooks::total::close_watcher",
    skip(self)
  )]
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
