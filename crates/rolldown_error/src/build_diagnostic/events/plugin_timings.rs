use std::fmt::Write as _;

use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

/// A row must reach this to be worth a line.
const MIN_ROW_MS: f64 = 1_000.0;
const MAX_MEASURED_ROWS: usize = 12;
const MAX_UNMEASURABLE_ROWS: usize = 3;

/// Where a measured callback was configured, so a report can tell a plugin's hook from a
/// user callback the core invokes directly. The warning below does not use it; it is
/// carried for the other consumers of the same measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTimingKind {
  Plugin,
  OutputOption,
  InputOption,
}

/// What one callback cost, measured inside it on the JavaScript side.
///
/// The bundler can only bracket dispatch and completion, and for a concurrently dispatched
/// hook that is mostly the queue the call waited in — so these numbers come from the other
/// side of the binding. See `packages/rolldown/src/utils/plugin-timings.ts`.
#[derive(Debug, Clone)]
pub struct PluginTiming {
  /// The plugin the callback belongs to, or the options it was configured on.
  pub owner: String,
  pub kind: PluginTimingKind,
  /// `transform`, `codeSplitting groups[].name`, … — what the user writes in their config.
  pub hook: String,
  pub calls: u32,
  /// Summed spans. Execution time only when [`PluginTiming::rankable`].
  pub ms: f64,
  pub max_in_flight: u32,
  /// How much of [`PluginTiming::ms`] is double counted because calls overlapped.
  pub overlap_ms: f64,
  /// Whether [`PluginTiming::ms`] may be compared against another row's. False when enough
  /// of this hook's calls overlapped that its total meaningfully double counts wall clock.
  pub rankable: bool,
}

/// One build's worth of JavaScript-side measurement.
#[derive(Debug, Clone, Default)]
pub struct PluginTimingsMeasurement {
  /// Wall time in which *any* measured callback was running, counting overlap once. Unlike
  /// a sum of spans this cannot outrun the build, which is what makes it usable as a share.
  pub busy_ms: f64,
  pub rows: Vec<PluginTiming>,
}

#[derive(Debug)]
pub struct PluginTimings {
  build_ms: f64,
  busy_ms: f64,
  /// Sorted by cost, capped. Every row here can be compared against the others.
  measured: Vec<PluginTiming>,
  /// Sorted by how heavily they overlapped, capped. Shown without numbers.
  unmeasurable: Vec<PluginTiming>,
  /// Rows the caps dropped, so a bounded view can say what it bounded.
  measured_hidden: usize,
  unmeasurable_hidden: usize,
}

impl PluginTimings {
  /// Select and order what is worth showing, or `None` when nothing is.
  ///
  /// Whether the build was plugin-bound at all is a separate question, answered before this
  /// by `BuildTimings::plugins_are_slow` — the clocks for it live on the Rust side.
  pub fn new(build_ms: f64, measurement: PluginTimingsMeasurement) -> Option<Self> {
    let (rankable, overlapped): (Vec<_>, Vec<_>) =
      measurement.rows.into_iter().partition(|row| row.rankable);

    let mut measured = rankable.into_iter().filter(|row| row.ms >= MIN_ROW_MS).collect::<Vec<_>>();
    measured.sort_by(|a, b| b.ms.total_cmp(&a.ms));
    let measured_hidden = measured.len().saturating_sub(MAX_MEASURED_ROWS);
    measured.truncate(MAX_MEASURED_ROWS);

    // Most-overlapped first: their spans mean least, so they are the ones most worth
    // pointing a profiler at.
    let mut unmeasurable = overlapped;
    unmeasurable.sort_by_key(|row| std::cmp::Reverse(row.max_in_flight));
    let unmeasurable_hidden = unmeasurable.len().saturating_sub(MAX_UNMEASURABLE_ROWS);
    unmeasurable.truncate(MAX_UNMEASURABLE_ROWS);

    if measured.is_empty() && unmeasurable.is_empty() {
      return None;
    }
    Some(Self {
      build_ms,
      busy_ms: measurement.busy_ms,
      measured,
      unmeasurable,
      measured_hidden,
      unmeasurable_hidden,
    })
  }
}

impl BuildEvent for PluginTimings {
  fn kind(&self) -> EventKind {
    EventKind::PluginTimings
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    const DOC_LINK: &str = "https://rolldown.rs/reference/InputOptions.checks#plugintimings";

    // `closeBundle` runs after the clock the build was measured with stopped, so a span can
    // outrun the build it is a share of. Clamp rather than print an impossible percentage.
    let share = |ms: f64| -> u64 {
      #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let percent = (ms / self.build_ms * 100.0).round() as u64;
      percent.min(100)
    };

    let mut out = format!(
      "Your build spent {}% of {} inside plugin hooks ({}).",
      share(self.busy_ms),
      format_duration(self.build_ms),
      format_duration(self.busy_ms)
    );

    if !self.measured.is_empty() {
      // Not "time they ran": excluding the dispatch queue is what measuring from inside the
      // callback buys, and it does not separate running from awaiting.
      out.push_str(
        "\nMeasured inside the callback, so queue time is excluded and time the callback \
         itself awaited is not:",
      );
      let mut listed_ms = 0.0;
      for row in &self.measured {
        listed_ms += row.ms;
        let _ = write!(
          out,
          "\n  - {} {} ({}%, {}, {} call{})",
          row.owner,
          row.hook,
          share(row.ms),
          format_duration(row.ms),
          row.calls,
          plural(row.calls)
        );
      }
      if self.measured_hidden > 0 {
        let _ = write!(out, "\n  … and {} more below 1s or past the cap", self.measured_hidden);
      }
      // Without this the gap between the headline and the rows is unexplained, and a reader
      // cannot tell whether it is unmeasurable callbacks or core work.
      if !self.unmeasurable.is_empty() || self.measured_hidden > 0 {
        let _ = write!(
          out,
          "\nThose rows are {}% of the build; the rest of the {}% is below.",
          share(listed_ms),
          share(self.busy_ms)
        );
      }
    }

    if !self.unmeasurable.is_empty() {
      let total = self.unmeasurable.len() + self.unmeasurable_hidden;
      let _ = write!(
        out,
        "\nNot measurable — {} hook{} whose calls overlap, so elapsed time covers work other \
         calls were doing. Profile with `node --cpu-prof`:",
        total,
        plural(u32::try_from(total).unwrap_or(u32::MAX))
      );
      for row in &self.unmeasurable {
        let _ =
          write!(out, "\n  - {} {} ({} call{})", row.owner, row.hook, row.calls, plural(row.calls));
      }
      if self.unmeasurable_hidden > 0 {
        let _ = write!(out, "\n  … and {} more", self.unmeasurable_hidden);
      }
    }

    let _ = write!(out, "\nSee {DOC_LINK} for more details.");
    out
  }
}

/// `1 call`, `2 calls`.
fn plural(count: u32) -> &'static str {
  if count == 1 { "" } else { "s" }
}

fn format_duration(ms: f64) -> String {
  if ms >= 1_000.0 { format!("{:.1}s", ms / 1_000.0) } else { format!("{}ms", ms.round()) }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn row(owner: &str, hook: &str, ms: f64, calls: u32, max_in_flight: u32) -> PluginTiming {
    PluginTiming {
      owner: owner.to_string(),
      kind: PluginTimingKind::Plugin,
      hook: hook.to_string(),
      calls,
      ms,
      max_in_flight,
      overlap_ms: 0.0,
      rankable: max_in_flight == 1,
    }
  }

  fn render(build_ms: f64, busy_ms: f64, rows: Vec<PluginTiming>) -> Option<String> {
    PluginTimings::new(build_ms, PluginTimingsMeasurement { busy_ms, rows })
      .map(|timings| timings.message(&DiagnosticOptions::default()))
  }

  #[test]
  fn ranks_measured_rows_and_names_the_ones_it_cannot_measure() {
    let message = render(
      10_000.0,
      8_000.0,
      vec![
        row("slow-plugin", "transform", 6_000.0, 500, 1),
        row("output options", "codeSplitting groups[].name", 2_000.0, 900, 1),
        row("async-plugin", "resolveId", 47_000.0, 3_000, 42),
        row("tiny-plugin", "buildStart", 5.0, 1, 1),
      ],
    )
    .unwrap();

    let rows = message.lines().filter(|line| line.starts_with("  - ")).collect::<Vec<_>>();
    assert_eq!(
      rows,
      vec![
        "  - slow-plugin transform (60%, 6.0s, 500 calls)",
        "  - output options codeSplitting groups[].name (20%, 2.0s, 900 calls)",
        "  - async-plugin resolveId (3000 calls)",
      ]
    );
    assert!(message.contains("80% of 10.0s"));
    // The 47s row would top any ranking, which is exactly why it gets no number.
    assert!(!message.contains("47.0s"));
    // Below the one-second floor.
    assert!(!message.contains("tiny-plugin"));
  }

  #[test]
  fn a_single_call_is_not_pluralised() {
    let message = render(10_000.0, 8_000.0, vec![row("p", "buildStart", 2_000.0, 1, 1)]).unwrap();
    assert!(message.contains("1 call)"), "{message}");
    assert!(!message.contains("1 calls"), "{message}");
  }

  #[test]
  fn says_what_the_listed_rows_add_up_to() {
    // Without this the gap between the headline and the rows is unexplained, and a reader
    // cannot tell whether it is unmeasurable callbacks or core work.
    let message = render(
      10_000.0,
      9_600.0,
      vec![row("p", "transform", 5_000.0, 3, 1), row("q", "resolveId", 4_000.0, 9, 12)],
    )
    .unwrap();
    assert!(message.contains("Those rows are 50% of the build"), "{message}");
    assert!(message.contains("96%"), "{message}");
  }

  #[test]
  fn says_how_many_rows_it_dropped() {
    // A bounded view has to say what it bounded.
    let measured = (0..u32::try_from(MAX_MEASURED_ROWS).unwrap() + 3)
      .map(|i| row("p", "transform", f64::from(2_000 + i), 1, 1));
    let overlapped = (0..u32::try_from(MAX_UNMEASURABLE_ROWS).unwrap() + 2)
      .map(|i| row("q", "resolveId", 9_000.0, 1, 2 + i));
    let message = render(100_000.0, 90_000.0, measured.chain(overlapped).collect()).unwrap();

    assert!(message.contains("… and 3 more below 1s or past the cap"), "{message}");
    assert!(message.contains("… and 2 more"), "{message}");
    assert!(message.contains("5 hooks whose calls overlap"), "{message}");
  }

  #[test]
  fn says_nothing_when_every_row_is_below_the_floor() {
    assert!(render(10_000.0, 8_000.0, vec![row("tiny", "buildStart", 5.0, 1, 1)]).is_none());
  }

  #[test]
  fn says_nothing_when_there_is_nothing_to_say() {
    assert!(render(10_000.0, 0.0, vec![]).is_none());
  }

  #[test]
  fn a_span_outrunning_the_build_is_clamped() {
    // `closeBundle` runs after the build clock stopped, so its span can exceed the build.
    let message =
      render(4_000.0, 6_000.0, vec![row("late", "closeBundle", 6_000.0, 1, 1)]).unwrap();
    assert!(message.contains("100% of 4.0s"));
    assert!(!message.contains("150%"));
  }
}
