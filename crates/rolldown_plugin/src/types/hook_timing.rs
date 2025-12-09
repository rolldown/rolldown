use std::sync::atomic::{AtomicU64, Ordering};

use arcstr::ArcStr;
use dashmap::DashMap;
use rolldown_common::PluginIdx;

/// Summary of timing for a single plugin
#[derive(Debug, Clone)]
pub struct PluginTimingSummary {
  pub plugin_name: ArcStr,
  pub total_duration_nanos: u64,
}

/// Plugin timing data containing name and accumulated duration
#[derive(Debug)]
struct PluginTimingData {
  name: ArcStr,
  duration_nanos: AtomicU64,
}

/// Collects timing information for plugin hooks
#[derive(Debug, Default)]
pub struct HookTimingCollector {
  /// Map from plugin_idx to timing data (only non-internal plugins are registered)
  plugins: DashMap<PluginIdx, PluginTimingData>,
  /// Total build time in nanoseconds
  total_build_nanos: AtomicU64,
  /// Link stage time in nanoseconds (pure Rust core, no plugins)
  link_stage_nanos: AtomicU64,
}

impl HookTimingCollector {
  /// Register a plugin with its name (only for non-internal plugins)
  pub fn register_plugin(&self, plugin_idx: PluginIdx, name: ArcStr) {
    self.plugins.insert(plugin_idx, PluginTimingData { name, duration_nanos: AtomicU64::new(0) });
  }

  /// Record a hook execution time (only records if plugin was registered)
  pub fn record(&self, plugin_idx: PluginIdx, nanos: u64) {
    if let Some(data) = self.plugins.get(&plugin_idx) {
      data.duration_nanos.fetch_add(nanos, Ordering::Relaxed);
    }
  }

  /// Set total build time in nanoseconds
  pub(crate) fn set_total_build_nanos(&self, nanos: u64) {
    self.total_build_nanos.store(nanos, Ordering::Relaxed);
  }

  /// Set link stage time in nanoseconds
  pub(crate) fn set_link_stage_nanos(&self, nanos: u64) {
    self.link_stage_nanos.store(nanos, Ordering::Relaxed);
  }

  /// Check if plugins are taking too much time.
  ///
  /// Returns `true` if plugin time (total - link stage) is more than 100x the link stage time.
  /// This works because plugins primarily run during the scan and generate stages, not the link stage.
  /// This 100x threshold was determined by studying plugin impact on real-world projects.
  #[expect(clippy::cast_precision_loss)]
  pub(crate) fn plugins_are_slow(&self) -> bool {
    let total = self.total_build_nanos.load(Ordering::Relaxed);
    let link = self.link_stage_nanos.load(Ordering::Relaxed);
    if total == 0 || link == 0 {
      return false;
    }
    (total - link) as f64 / link as f64 > 100.0
  }

  /// Get timing summary for all plugins
  pub fn get_summary(&self) -> Vec<PluginTimingSummary> {
    let mut summaries = self
      .plugins
      .iter()
      .map(|entry| {
        let total_duration_nanos = entry.value().duration_nanos.load(Ordering::Relaxed);
        PluginTimingSummary { plugin_name: entry.value().name.clone(), total_duration_nanos }
      })
      .collect::<Vec<_>>();
    summaries.sort_by(|a, b| b.total_duration_nanos.cmp(&a.total_duration_nanos));
    summaries
  }

  /// Clear all collected timings
  pub fn clear(&self) {
    for mut entry in self.plugins.iter_mut() {
      entry.value_mut().duration_nanos.store(0, Ordering::Relaxed);
    }
  }
}
