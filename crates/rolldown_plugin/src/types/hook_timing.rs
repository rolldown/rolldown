use std::sync::atomic::{AtomicU64, Ordering};

use arcstr::ArcStr;
use dashmap::DashMap;
use rolldown_common::PluginIdx;

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
  plugins: DashMap<PluginIdx, PluginTimingData>,
  /// Total build time in microseconds
  total_build_micros: AtomicU64,
  /// Link stage time in microseconds (pure Rust core, no plugins)
  link_stage_micros: AtomicU64,
  /// Accumulated time (microseconds) for the `output.codeSplitting` / `advancedChunks`
  /// `groups[].name` chunk-name classifier — a user JS callback invoked directly from the
  /// Rust core (NOT a plugin hook), so it is invisible to per-plugin timing yet can
  /// dominate a build. Surfaced as its own row in the `[PLUGIN_TIMINGS]` report. The set of
  /// such core-invoked output callbacks is known statically, so each gets a fixed field
  /// rather than a dynamic map (precedent: `link_stage_micros`).
  code_splitting_name_micros: AtomicU64,
}

impl HookTimingCollector {
  /// Register a plugin with its name (only for non-internal plugins)
  pub fn register_plugin(&self, plugin_idx: PluginIdx, name: ArcStr) {
    self.plugins.insert(plugin_idx, PluginTimingData { name, duration_micros: AtomicU64::new(0) });
  }

  /// Record a hook execution time in microseconds (only records if plugin was registered)
  pub fn record(&self, plugin_idx: PluginIdx, micros: u64) {
    if let Some(data) = self.plugins.get(&plugin_idx) {
      data.duration_micros.fetch_add(micros, Ordering::Relaxed);
    }
  }

  /// Accumulate execution time (microseconds) for the `output.codeSplitting` /
  /// `advancedChunks` `groups[].name` chunk-name classifier.
  pub fn record_code_splitting_name(&self, micros: u64) {
    self.code_splitting_name_micros.fetch_add(micros, Ordering::Relaxed);
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
      .iter()
      .map(|entry| {
        let total_duration_micros = entry.value().duration_micros.load(Ordering::Relaxed);
        PluginTimingSummary { plugin_name: entry.value().name.clone(), total_duration_micros }
      })
      .collect::<Vec<_>>();
    summaries.sort_by_key(|b| std::cmp::Reverse(b.total_duration_micros));
    summaries
  }

  /// Get timing summaries for non-plugin output-option callbacks invoked from the Rust
  /// core (reuses the `PluginTimingSummary` shape, with a stable label in `plugin_name`).
  /// Only callbacks that actually ran are included.
  pub fn get_output_callback_summary(&self) -> Vec<PluginTimingSummary> {
    let mut summaries = Vec::new();
    let code_splitting_name = self.code_splitting_name_micros.load(Ordering::Relaxed);
    if code_splitting_name > 0 {
      summaries.push(PluginTimingSummary {
        plugin_name: arcstr::literal!("output.codeSplitting groups[].name"),
        total_duration_micros: code_splitting_name,
      });
    }
    summaries
  }

  /// Clear all collected timings
  pub fn clear(&self) {
    for mut entry in self.plugins.iter_mut() {
      entry.value_mut().duration_micros.store(0, Ordering::Relaxed);
    }
    self.code_splitting_name_micros.store(0, Ordering::Relaxed);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn output_callback_timing_accumulates() {
    let collector = HookTimingCollector::default();

    // Nothing ran yet → no row.
    assert!(collector.get_output_callback_summary().is_empty());

    collector.record_code_splitting_name(1_000);
    collector.record_code_splitting_name(2_500);

    let summary = collector.get_output_callback_summary();
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].plugin_name.as_str(), "output.codeSplitting groups[].name");
    assert_eq!(summary[0].total_duration_micros, 3_500);

    collector.clear();
    assert!(collector.get_output_callback_summary().is_empty());
  }
}
