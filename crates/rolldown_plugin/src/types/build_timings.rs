use std::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

/// The two wall clocks that decide whether a build was plugin-bound.
///
/// Kept apart from [`crate::HookTimingCollector`], which measures what individual plugins
/// cost: these are properties of the build itself, they are set once each rather than
/// accumulated per call, and the question they answer — "is this worth reporting on at
/// all?" — is asked before any per-plugin number is looked at.
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

  /// Whether the build looks plugin-bound: over `MIN_BUILD_MICROS` long, with non-link time
  /// more than `PLUGIN_TIME_OVER_LINK_TIME` times the link stage.
  ///
  /// This works because plugins run during the scan and generate stages, not the link
  /// stage, which makes the link stage the one stretch of a build with no plugin in it. The
  /// multiplier was settled by studying plugin impact on real-world projects, and the
  /// minimum build time keeps fast builds quiet.
  pub fn plugins_are_slow(&self) -> bool {
    Self::is_plugin_bound(self.total_micros(), self.link_stage_micros())
  }

  /// The same test over totals supplied by the caller, so a build that produced several
  /// outputs can sum its clocks before asking.
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
