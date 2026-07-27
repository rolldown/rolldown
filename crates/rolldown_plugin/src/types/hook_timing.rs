use std::sync::atomic::{AtomicU64, Ordering};

use arcstr::ArcStr;
use dashmap::DashMap;
use rolldown_common::PluginIdx;

use super::hook_kind::{HookKind, TimingSection};

/// The estimated wall-clock time one hook of one owner cost the build.
#[derive(Debug, Clone)]
pub struct HookTimingEstimate {
  /// The plugin the hook belongs to, or `None` for callbacks the Rust core invokes
  /// directly rather than through a plugin.
  ///
  /// This is the plugin's identity, not its name. Two registrations of the same
  /// plugin are distinct owners even though they display identically, so callers
  /// must aggregate on this and resolve a name only for display — see
  /// [`HookTimingCollector::owner_name`].
  pub owner: Option<PluginIdx>,
  pub hook: HookKind,
  pub estimated_micros: u64,
}

/// Collects timing information for plugin hooks.
#[derive(Debug, Default)]
pub struct HookTimingCollector {
  /// Names of the plugins being timed. Only non-internal plugins are registered, and
  /// only registered plugins are recorded.
  plugin_names: DashMap<PluginIdx, ArcStr>,
  /// Measured hook time. A `None` owner is a callback the Rust core invokes directly
  /// rather than through a plugin.
  hooks: DashMap<(Option<PluginIdx>, HookKind), AtomicU64>,
  /// Measured wall-clock span of each section, indexed by `TimingSection as usize`.
  /// Only sections that run hooks concurrently have one; see [`TimingSection`].
  section_micros: [AtomicU64; TimingSection::COUNT],
  /// Total build time in microseconds
  total_build_micros: AtomicU64,
  /// Link stage time in microseconds (pure Rust core, no plugins)
  link_stage_micros: AtomicU64,
}

impl HookTimingCollector {
  /// Register a plugin with its name (only for non-internal plugins)
  pub fn register_plugin(&self, plugin_idx: PluginIdx, name: ArcStr) {
    self.plugin_names.insert(plugin_idx, name);
  }

  /// Accumulate a hook's measured execution time in microseconds. `owner` is `None`
  /// for callbacks the Rust core invokes directly.
  ///
  /// Every hook is recorded, including internal `builtin:` plugins that are never
  /// reported. Their work is part of the section wall clock being apportioned, so
  /// leaving it out of the denominator would hand their share to whichever user plugin
  /// happens to be measured — a 1ms user `transform` beside a 5s builtin one would be
  /// billed the entire phase. [`Self::estimate`] drops them from its rows instead, which
  /// withholds their share rather than redistributing it.
  pub fn record(&self, owner: Option<PluginIdx>, hook: HookKind, micros: u64) {
    let key = (owner, hook);
    // Take the read path once the slot exists, which is every call after the first
    // for a given (owner, hook) pair.
    if let Some(slot) = self.hooks.get(&key) {
      slot.fetch_add(micros, Ordering::Relaxed);
      return;
    }
    self.hooks.entry(key).or_default().fetch_add(micros, Ordering::Relaxed);
  }

  /// Accumulate the measured wall-clock span of a section that runs hooks
  /// concurrently. Spans add up rather than replace, because a section can run more
  /// than once per build.
  pub(crate) fn record_section_micros(&self, section: TimingSection, micros: u64) {
    self.section_micros[section as usize].fetch_add(micros, Ordering::Relaxed);
  }

  /// Set total build time in microseconds
  pub(crate) fn set_total_build_micros(&self, micros: u64) {
    self.total_build_micros.store(micros, Ordering::Relaxed);
  }

  /// Set link stage time in microseconds
  pub(crate) fn set_link_stage_micros(&self, micros: u64) {
    self.link_stage_micros.store(micros, Ordering::Relaxed);
  }

  /// Total build time in microseconds, the denominator the report's percentages are
  /// shares of.
  pub(crate) fn total_build_micros(&self) -> u64 {
    self.total_build_micros.load(Ordering::Relaxed)
  }

  /// Check if plugins are taking too much time.
  ///
  /// Returns `true` if plugin time (total - link stage) is more than 100x the link stage time.
  /// This works because plugins primarily run during the scan and generate stages, not the link stage.
  /// This 100x threshold was determined by studying plugin impact on real-world projects.
  ///
  /// To avoid noisy warnings for fast builds, the warning only triggers when total build time exceeds 3 seconds.
  ///
  /// This gate stays independent of [`Self::estimate`] on purpose. The estimate
  /// apportions a section's whole wall clock to the hooks in it, so it cannot itself
  /// tell a plugin-bound phase from a Rust-bound one; this comparison against the
  /// pure-Rust link stage is what establishes that plugins dominate before the
  /// estimate is used to say by how much.
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

  /// Estimate the wall-clock time each reportable owner's hooks cost the build.
  ///
  /// See [`TimingSection`] for why measured hook time is not comparable across
  /// sections and how the estimate corrects for it.
  ///
  /// The denominator spans every recorded hook, but only registered plugins get rows:
  /// a user cannot act on a `builtin:` plugin, so its share of the phase is withheld
  /// from the report rather than handed to someone who did not spend it.
  pub fn estimate(&self) -> Vec<HookTimingEstimate> {
    let mut section_hook_micros = [0u64; TimingSection::COUNT];
    for entry in &self.hooks {
      let section = entry.key().1.section();
      section_hook_micros[section as usize] += entry.value().load(Ordering::Relaxed);
    }

    self
      .hooks
      .iter()
      .filter_map(|entry| {
        let (owner, hook) = *entry.key();
        let measured = entry.value().load(Ordering::Relaxed);
        if measured == 0 {
          return None;
        }
        // Counted above, reported nowhere. See the note on this method.
        if owner.is_some_and(|plugin_idx| !self.plugin_names.contains_key(&plugin_idx)) {
          return None;
        }
        let estimated_micros = match hook.section() {
          // Fires outside the build window, so it is not part of the time the report
          // divides by.
          TimingSection::OutsideBuild => return None,
          // Ran one call at a time, so the measurement is already the elapsed time.
          TimingSection::Serial => measured,
          section => {
            let wall_micros = self.section_micros[section as usize].load(Ordering::Relaxed);
            // Without a recorded boundary there is no wall clock to apportion, and a
            // row derived from one would be invented rather than measured.
            if wall_micros == 0 {
              return None;
            }
            apportion(measured, section_hook_micros[section as usize], wall_micros)
          }
        };
        Some(HookTimingEstimate { owner, hook, estimated_micros })
      })
      .collect()
  }

  /// The name to display for an owner. Callbacks the Rust core invokes directly are
  /// still user code, just configured on the output options rather than in a plugin.
  ///
  /// Only for rendering — never key on this. Distinct plugins can share a name, and a
  /// plugin is free to be named the same as the core label.
  pub fn owner_name(&self, owner: Option<PluginIdx>) -> ArcStr {
    owner.map_or_else(
      || arcstr::literal!("output options"),
      |plugin_idx| self.plugin_names.get(&plugin_idx).map_or_else(ArcStr::new, |name| name.clone()),
    )
  }

  /// Clear all collected timings, keeping plugin registrations.
  pub fn clear(&self) {
    self.hooks.clear();
    for section in &self.section_micros {
      section.store(0, Ordering::Relaxed);
    }
  }
}

/// `measured / section_total * wall_micros`, the share of a section's real elapsed
/// time that one hook's measured time accounts for.
#[expect(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn apportion(measured: u64, section_total: u64, wall_micros: u64) -> u64 {
  debug_assert!(
    measured <= section_total,
    "a hook's measured time is part of its section's total, so the share cannot exceed 1"
  );
  (measured as f64 / section_total as f64 * wall_micros as f64) as u64
}

#[cfg(test)]
mod tests {
  use super::*;

  fn owned(collector: &HookTimingCollector, hook: HookKind) -> u64 {
    collector
      .estimate()
      .into_iter()
      .find(|estimate| estimate.hook == hook)
      .map_or(0, |estimate| estimate.estimated_micros)
  }

  #[test]
  fn serial_hooks_are_reported_as_measured() {
    let collector = HookTimingCollector::default();

    collector.record(None, HookKind::CodeSplittingName, 1_000);
    collector.record(None, HookKind::CodeSplittingName, 2_500);

    let estimates = collector.estimate();
    assert_eq!(estimates.len(), 1);
    assert_eq!(collector.owner_name(estimates[0].owner).as_str(), "output options");
    // Serial call sites need no boundary: the sum is already the elapsed time.
    assert_eq!(estimates[0].estimated_micros, 3_500);
  }

  #[test]
  fn concurrent_hooks_are_scaled_to_the_section_wall_clock() {
    let collector = HookTimingCollector::default();

    // Overlapping calls, so the measured sum (10s) far exceeds the 4s the section
    // actually took. Shares within the section are what survive: 75% / 25%.
    collector.record(None, HookKind::Transform, 7_500_000);
    collector.record(None, HookKind::Load, 2_500_000);
    collector.record_section_micros(TimingSection::FetchModule, 4_000_000);

    assert_eq!(owned(&collector, HookKind::Transform), 3_000_000);
    assert_eq!(owned(&collector, HookKind::Load), 1_000_000);
  }

  #[test]
  fn a_serial_hook_is_not_drowned_out_by_an_overlapping_section() {
    let collector = HookTimingCollector::default();

    // The classifier costs more real time than module loading, but is measured
    // serially while the per-module hooks inflate 10x by queueing behind each other.
    collector.record(None, HookKind::Transform, 20_000_000);
    collector.record_section_micros(TimingSection::FetchModule, 2_000_000);
    collector.record(None, HookKind::CodeSplittingName, 6_000_000);

    assert!(
      owned(&collector, HookKind::CodeSplittingName) > owned(&collector, HookKind::Transform)
    );
  }

  #[test]
  fn sections_without_a_measured_boundary_are_dropped() {
    let collector = HookTimingCollector::default();

    // Never ran to completion, so there is no wall clock to apportion.
    collector.record(None, HookKind::RenderChunk, 5_000);
    // Fires outside the build window.
    collector.record(None, HookKind::WatchChange, 5_000);

    assert!(collector.estimate().is_empty());
  }

  #[test]
  fn clear_keeps_registrations_and_drops_measurements() {
    let collector = HookTimingCollector::default();

    collector.record(None, HookKind::CodeSplittingName, 1_000);
    collector.record_section_micros(TimingSection::FetchModule, 1_000);
    collector.clear();

    assert!(collector.estimate().is_empty());
    assert_eq!(
      collector.section_micros[TimingSection::FetchModule as usize].load(Ordering::Relaxed),
      0
    );
  }

  #[test]
  fn unregistered_plugins_get_no_row() {
    let collector = HookTimingCollector::default();
    let plugin_idx = PluginIdx::from_raw(0);

    collector.record(Some(plugin_idx), HookKind::CodeSplittingName, 1_000);
    assert!(collector.estimate().is_empty());

    collector.register_plugin(plugin_idx, arcstr::literal!("my-plugin"));

    let estimates = collector.estimate();
    assert_eq!(estimates.len(), 1);
    assert_eq!(collector.owner_name(estimates[0].owner).as_str(), "my-plugin");
  }

  #[test]
  fn builtin_plugins_keep_their_share_of_the_section() {
    let collector = HookTimingCollector::default();
    let user = PluginIdx::from_raw(0);
    let builtin = PluginIdx::from_raw(1);
    // Only the user plugin is registered; `builtin:` names never are.
    collector.register_plugin(user, arcstr::literal!("my-plugin"));

    collector.record(Some(user), HookKind::Transform, 1_000);
    collector.record(Some(builtin), HookKind::Transform, 9_000);
    collector.record_section_micros(TimingSection::FetchModule, 10_000);

    // The builtin did nine tenths of the measured work. Excluding it from the
    // denominator would bill the user plugin for the whole phase.
    let estimates = collector.estimate();
    assert_eq!(estimates.len(), 1);
    assert_eq!(estimates[0].owner, Some(user));
    assert_eq!(estimates[0].estimated_micros, 1_000);
  }

  #[test]
  fn same_named_plugins_stay_distinct_owners() {
    let collector = HookTimingCollector::default();
    let first = PluginIdx::from_raw(0);
    let second = PluginIdx::from_raw(1);
    collector.register_plugin(first, arcstr::literal!("my-plugin"));
    collector.register_plugin(second, arcstr::literal!("my-plugin"));

    collector.record(Some(first), HookKind::Transform, 1_000);
    collector.record(Some(second), HookKind::Transform, 3_000);
    collector.record_section_micros(TimingSection::FetchModule, 4_000);

    // Two registrations of one plugin are separate culprits. Keying on the display
    // name would fuse them into a single 4_000 row that neither earned.
    let mut estimates = collector.estimate();
    estimates.sort_by_key(|estimate| estimate.estimated_micros);
    assert_eq!(estimates.len(), 2);
    assert_eq!(estimates[0].owner, Some(first));
    assert_eq!(estimates[0].estimated_micros, 1_000);
    assert_eq!(estimates[1].owner, Some(second));
    assert_eq!(estimates[1].estimated_micros, 3_000);
  }

  #[test]
  fn transform_ast_takes_its_share_of_module_loading() {
    let collector = HookTimingCollector::default();

    // `transformAst` runs inside `fetch_modules`. Leaving it untimed would not omit
    // its cost — the whole section wall clock is apportioned regardless, so its time
    // would be handed to `transform` instead.
    collector.record(None, HookKind::Transform, 3_000);
    collector.record(None, HookKind::TransformAst, 1_000);
    collector.record_section_micros(TimingSection::FetchModule, 8_000);

    assert_eq!(owned(&collector, HookKind::Transform), 6_000);
    assert_eq!(owned(&collector, HookKind::TransformAst), 2_000);
  }
}
