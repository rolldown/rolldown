use std::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

/// The two wall clocks that decide whether a build was plugin-bound.
///
/// Plugin hooks themselves are timed on the JavaScript side, because this side can only
/// bracket dispatch and completion and for a concurrently dispatched hook that is mostly
/// queue wait — see `packages/rolldown/src/utils/plugin-timings.ts`. But the *link stage*
/// is visible only from here, and it is the one stretch of a build that is pure core work
/// with no plugin in it. Non-link time running far ahead of link time is what says the
/// build is plugin-bound, so these are handed across the binding for the JavaScript side
/// to gate its report on.
#[derive(Debug, Default)]
pub struct BuildTimings {
  total_micros: AtomicU64,
  link_stage_micros: AtomicU64,
}

impl BuildTimings {
  pub fn set_total(&self, elapsed: Duration) {
    self.total_micros.store(Self::micros(elapsed), Ordering::Relaxed);
  }

  pub fn set_link_stage(&self, elapsed: Duration) {
    self.link_stage_micros.store(Self::micros(elapsed), Ordering::Relaxed);
  }

  /// Zero until [`Self::set_total`] runs, which only the full `write`/`generate` paths do.
  /// Watch, incremental and dev builds go straight to `bundle_write`/`bundle_generate`, so
  /// they leave this at zero and nothing is reported for them.
  pub fn total_micros(&self) -> u64 {
    self.total_micros.load(Ordering::Relaxed)
  }

  pub fn link_stage_micros(&self) -> u64 {
    self.link_stage_micros.load(Ordering::Relaxed)
  }

  /// Whether the build looks plugin-bound: over `MIN_BUILD` long, with non-link time more
  /// than `PLUGIN_TIME_OVER_LINK_TIME` times the link stage.
  ///
  /// The link stage is the one stretch of a build with no plugin in it, which is what makes
  /// it a baseline. The multiplier was settled by studying real-world projects.
  ///
  /// This only decides whether to *look*; what the build actually spent in plugin callbacks
  /// is measured on the JavaScript side, because this side cannot see when a callback began
  /// running — see `packages/rolldown/src/utils/plugin-timings.ts`.
  pub fn plugins_are_slow(&self) -> bool {
    Self::is_plugin_bound(self.total_micros(), self.link_stage_micros())
  }

  /// The same test over totals supplied by the caller, so a build that produced several
  /// outputs can sum its clocks before asking. The measurement it is judged against
  /// accumulates across outputs, so these have to as well.
  #[expect(clippy::cast_precision_loss)]
  pub fn is_plugin_bound(total_micros: u64, link_stage_micros: u64) -> bool {
    const MIN_BUILD_MICROS: u64 = 3_000_000;
    const PLUGIN_TIME_OVER_LINK_TIME: f64 = 100.0;

    if total_micros < MIN_BUILD_MICROS || link_stage_micros == 0 || link_stage_micros > total_micros
    {
      return false;
    }
    (total_micros - link_stage_micros) as f64 / link_stage_micros as f64
      > PLUGIN_TIME_OVER_LINK_TIME
  }

  #[expect(clippy::cast_possible_truncation)]
  fn micros(elapsed: Duration) -> u64 {
    elapsed.as_micros() as u64
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn timings(total_micros: u64, link_micros: u64) -> BuildTimings {
    let timings = BuildTimings::default();
    timings.set_total(Duration::from_micros(total_micros));
    timings.set_link_stage(Duration::from_micros(link_micros));
    timings
  }

  #[test]
  fn a_fast_build_is_never_slow() {
    // However lopsided the ratio, two seconds of build is not worth interrupting anyone for.
    assert!(!timings(2_000_000, 1).plugins_are_slow());
  }

  #[test]
  fn a_build_whose_link_stage_is_not_dwarfed_is_not_plugin_bound() {
    // 60s build, 1s link: non-link time is 59x link time, under the bar.
    assert!(!timings(60_000_000, 1_000_000).plugins_are_slow());
    assert!(timings(60_000_000, 500_000).plugins_are_slow());
  }

  #[test]
  fn a_build_that_never_ran_is_not_slow() {
    // Watch, incremental and dev builds never set the total, and a link stage cannot be
    // zero-length in a build that happened — either way there is no ratio to take.
    assert!(!BuildTimings::default().plugins_are_slow());
    assert!(!timings(10_000_000, 0).plugins_are_slow());
    assert!(!timings(1_000, 10_000).plugins_are_slow());
  }
}
