use std::sync::atomic::{AtomicU64, Ordering};

use arcstr::ArcStr;
use rolldown_common::PluginIdx;
use rolldown_utils::dashmap::FxDashMap;

/// Summary of timing for a single plugin
#[derive(Debug, Clone)]
pub struct PluginTimingSummary {
  pub plugin_name: ArcStr,
  pub total_duration_micros: u64,
}

/// Plugin timing data containing name and accumulated duration
#[derive(Debug)]
struct PluginTimingData {
  name: ArcStr,
  duration_micros: AtomicU64,
}

/// Collects timing information for plugin hooks
#[derive(Debug, Default)]
pub struct HookTimingCollector {
  /// Map from plugin_idx to timing data (only non-internal plugins are registered)
  plugins: FxDashMap<PluginIdx, PluginTimingData>,
  /// Total build time in microseconds
  total_build_micros: AtomicU64,
  /// Link stage time in microseconds (pure Rust core, no plugins)
  link_stage_micros: AtomicU64,
}

impl HookTimingCollector {
  /// Register a plugin with its name (only for non-internal plugins)
  pub fn register_plugin(&self, plugin_idx: PluginIdx, name: ArcStr) {
    self
      .plugins
      .pin()
      .insert(plugin_idx, PluginTimingData { name, duration_micros: AtomicU64::new(0) });
  }

  /// Record a hook execution time in microseconds (only records if plugin was registered)
  pub fn record(&self, plugin_idx: PluginIdx, micros: u64) {
    if let Some(data) = self.plugins.pin().get(&plugin_idx) {
      data.duration_micros.fetch_add(micros, Ordering::Relaxed);
    }
  }

  /// Set total build time in microseconds
  pub(crate) fn set_total_build_micros(&self, micros: u64) {
    self.total_build_micros.store(micros, Ordering::Relaxed);
  }

  /// Set link stage time in microseconds
  pub(crate) fn set_link_stage_micros(&self, micros: u64) {
    self.link_stage_micros.store(micros, Ordering::Relaxed);
  }

  /// Check if plugins are taking too much time.
  ///
  /// Returns `true` if plugin time (total - link stage) is more than 100x the link stage time.
  /// This works because plugins primarily run during the scan and generate stages, not the link stage.
  /// This 100x threshold was determined by studying plugin impact on real-world projects.
  ///
  /// To avoid noisy warnings for fast builds, the warning only triggers when total build time exceeds 3 seconds.
  #[expect(clippy::cast_precision_loss)]
  pub(crate) fn plugins_are_slow(&self) -> bool {
    const MIN_BUILD_TIME_MICROS: u64 = 3_000_000;
    let total = self.total_build_micros.load(Ordering::Relaxed);
    let link = self.link_stage_micros.load(Ordering::Relaxed);
    if total == 0 || link == 0 || link > total || total < MIN_BUILD_TIME_MICROS {
      return false;
    }
    (total - link) as f64 / link as f64 > 100.0
  }

  /// Get timing summary for all plugins
  pub fn get_summary(&self) -> Vec<PluginTimingSummary> {
    let mut summaries = self
      .plugins
      .pin()
      .iter()
      .map(|(_, data)| {
        let total_duration_micros = data.duration_micros.load(Ordering::Relaxed);
        PluginTimingSummary { plugin_name: data.name.clone(), total_duration_micros }
      })
      .collect::<Vec<_>>();
    summaries.sort_by_key(|b| std::cmp::Reverse(b.total_duration_micros));
    summaries
  }

  /// Clear all collected timings
  pub fn clear(&self) {
    let plugins = self.plugins.pin();
    for (_, data) in &plugins {
      data.duration_micros.store(0, Ordering::Relaxed);
    }
  }
}
