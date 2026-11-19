use std::{
  sync::atomic::{AtomicU64, Ordering},
  time::{Duration, Instant},
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
///
/// A build nobody is measuring keeps no clock at all: [`Self::start`] hands back `None`,
/// and every recorder is a no-op on it. Whether it is measured is decided once, where the
/// driver is built, from the same `checks.bundlerTimings` that turns the JavaScript side's
/// measurement on — the two have to agree, because a report needs both halves.
#[derive(Debug, Default)]
pub struct BuildTimings {
  measured: bool,
  total_micros: AtomicU64,
  link_stage_micros: AtomicU64,
}

impl BuildTimings {
  pub fn new(measured: bool) -> Self {
    Self { measured, ..Self::default() }
  }

  /// The instant to measure a stretch of the build from, or `None` when it is not being
  /// measured — then nothing downstream reads the clock, so there is no reason to take one.
  ///
  /// Pass what this returns to [`Self::record_total`] or [`Self::record_link_stage`].
  pub fn start(&self) -> Option<Instant> {
    self.measured.then(Instant::now)
  }

  pub fn record_total(&self, start: Option<Instant>) {
    if let Some(start) = start {
      self.set_total(start.elapsed());
    }
  }

  pub fn record_link_stage(&self, start: Option<Instant>) {
    if let Some(start) = start {
      self.set_link_stage(start.elapsed());
    }
  }

  fn set_total(&self, elapsed: Duration) {
    self.total_micros.store(Self::micros(elapsed), Ordering::Relaxed);
  }

  fn set_link_stage(&self, elapsed: Duration) {
    self.link_stage_micros.store(Self::micros(elapsed), Ordering::Relaxed);
  }

  /// Zero until [`Self::record_total`] runs, which only the full `write`/`generate` paths
  /// do, and only when the build is being measured. Watch, incremental and dev builds go
  /// straight to `bundle_write`/`bundle_generate`, so they leave this at zero and nothing
  /// is reported for them.
  pub fn total_micros(&self) -> u64 {
    self.total_micros.load(Ordering::Relaxed)
  }

  pub fn link_stage_micros(&self) -> u64 {
    self.link_stage_micros.load(Ordering::Relaxed)
  }

  /// The totals are supplied by the caller, so a build that produced several outputs can sum
  /// its clocks before asking. The measurement it is judged against accumulates across
  /// outputs, so these have to as well.
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

  #[test]
  fn a_fast_build_is_never_slow() {
    // However lopsided the ratio, two seconds of build is not worth interrupting anyone for.
    assert!(!BuildTimings::is_plugin_bound(2_000_000, 1));
  }

  #[test]
  fn a_build_whose_link_stage_is_not_dwarfed_is_not_plugin_bound() {
    // 60s build, 1s link: non-link time is 59x link time, under the bar.
    assert!(!BuildTimings::is_plugin_bound(60_000_000, 1_000_000));
    assert!(BuildTimings::is_plugin_bound(60_000_000, 500_000));
  }

  #[test]
  fn a_build_that_never_ran_is_not_slow() {
    // Watch, incremental and dev builds never set the total, and a link stage cannot be
    // zero-length in a build that happened — either way there is no ratio to take.
    assert!(!BuildTimings::is_plugin_bound(0, 0));
    assert!(!BuildTimings::is_plugin_bound(10_000_000, 0));
    assert!(!BuildTimings::is_plugin_bound(1_000, 10_000));
  }

  #[test]
  fn a_build_that_is_not_measured_keeps_no_clock() {
    // Nobody will ask what it cost, so there is nothing to take: `start` hands back no
    // instant, and the recorders it feeds have nothing to store.
    let timings = BuildTimings::new(false);
    assert!(timings.start().is_none());
    timings.record_total(timings.start());
    timings.record_link_stage(timings.start());
    assert_eq!(timings.total_micros(), 0);
    assert_eq!(timings.link_stage_micros(), 0);
  }

  #[test]
  fn a_measured_build_starts_a_clock() {
    assert!(BuildTimings::new(true).start().is_some());
  }
}
