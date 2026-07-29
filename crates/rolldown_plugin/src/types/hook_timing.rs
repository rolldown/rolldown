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

/// Merge every output's summaries and pick the rows worth showing, or `None` when none are.
///
/// Whether the build was plugin-bound at all is asked before this, from
/// [`crate::BuildTimings`] — these numbers only decide *what* to name once it has been.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
pub fn plugin_timings_info(
  summaries: Vec<PluginTimingSummary>,
) -> Option<Vec<rolldown_error::PluginTimingInfo>> {
  const MAX_ROWS: usize = 5;
  const ONE_SECOND_MICROS: u64 = 1_000_000;

  // Merged by name: one build may run several outputs, each with its own driver and so its
  // own `PluginIdx`, and the same plugin's work across them is one cost to the user.
  let mut merged: Vec<PluginTimingSummary> = Vec::new();
  for summary in summaries {
    match merged.iter_mut().find(|existing| existing.plugin_name == summary.plugin_name) {
      Some(existing) => existing.total_duration_micros += summary.total_duration_micros,
      None => merged.push(summary),
    }
  }

  let total_micros: u64 = merged.iter().map(|s| s.total_duration_micros).sum();
  if merged.is_empty() || total_micros == 0 {
    return None;
  }
  merged.sort_by_key(|s| std::cmp::Reverse(s.total_duration_micros));
  let threshold = (total_micros / merged.len() as u64).max(ONE_SECOND_MICROS);
  let result = merged
    .iter()
    .filter(|s| s.total_duration_micros >= threshold)
    .take(MAX_ROWS)
    .map(|s| rolldown_error::PluginTimingInfo {
      name: s.plugin_name.to_string(),
      percent: (s.total_duration_micros as f64 / total_micros as f64 * 100.0).round() as u8,
    })
    .collect::<Vec<_>>();
  if result.is_empty() { None } else { Some(result) }
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

  fn summary(name: &str, micros: u64) -> PluginTimingSummary {
    PluginTimingSummary { plugin_name: ArcStr::from(name), total_duration_micros: micros }
  }

  #[test]
  fn one_plugin_across_two_outputs_is_one_row() {
    // Each output has its own driver, so the same plugin arrives twice. Left unmerged it
    // would compete with itself for the row cap and halve its own share.
    let rows = plugin_timings_info(vec![
      summary("slow", 3_000_000),
      summary("slow", 3_000_000),
      summary("fast", 1_000_000),
    ])
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "slow");
    assert_eq!(rows[0].percent, 86);
  }

  #[test]
  fn nothing_worth_naming_is_no_report() {
    assert!(plugin_timings_info(vec![]).is_none());
    assert!(plugin_timings_info(vec![summary("quick", 10)]).is_none());
  }
}
