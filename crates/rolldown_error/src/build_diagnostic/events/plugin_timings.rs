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
  /// Wall time in which at least one measured callback ran. Overlap counts once, so the
  /// value cannot be more than the window the callbacks ran in. That is what makes it
  /// usable as a share. It can be more than the core build clock, because `closeBundle`
  /// runs after that clock stops.
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
  /// Rankable rows under [`MIN_ROW_MS`]. The message never shows them. The remainder
  /// sentence names them only when they exist.
  below_floor: usize,
}

impl PluginTimings {
  /// Select and order what is worth showing, or `None` when nothing is.
  ///
  /// Whether the build was plugin-bound at all is a separate question, answered before this
  /// by `BuildTimings::plugins_are_slow` — the clocks for it live on the Rust side.
  pub fn new(build_ms: f64, measurement: PluginTimingsMeasurement) -> Option<Self> {
    let (rankable, overlapped): (Vec<_>, Vec<_>) =
      measurement.rows.into_iter().partition(|row| row.rankable);

    let (mut measured, below_floor): (Vec<_>, Vec<_>) =
      rankable.into_iter().partition(|row| row.ms >= MIN_ROW_MS);
    let below_floor = below_floor.len();
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
      below_floor,
    })
  }
}

impl BuildEvent for PluginTimings {
  fn kind(&self) -> EventKind {
    EventKind::PluginTimings
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    const DOC_LINK: &str = "https://rolldown.rs/reference/InputOptions.checks#plugintimings";

    #[expect(clippy::cast_possible_truncation)]
    let percent = |ms: f64| -> i64 { (ms / self.build_ms * 100.0).round() as i64 };
    // `closeBundle` runs after the clock the build was measured with stopped, so a span can
    // outrun the build it is a share of. Clamp rather than print an impossible percentage.
    let share = |ms: f64| -> i64 { percent(ms).min(100) };

    let mut out = if self.busy_ms > self.build_ms {
      // Without this branch, the headline reads "7.4s of this 7.1s build". That looks like
      // a bug.
      format!(
        "Plugin hooks ran for {}. The build took {}. Hooks such as `closeBundle` run after \
         the build ends, so hook time can be more than build time.",
        format_duration(self.busy_ms),
        format_duration(self.build_ms)
      )
    } else {
      format!(
        "Plugin hooks ran for {} of this {} build ({}%).",
        format_duration(self.busy_ms),
        format_duration(self.build_ms),
        share(self.busy_ms)
      )
    };

    if !self.measured.is_empty() {
      // Not "time they ran": excluding the dispatch queue is what measuring from inside the
      // callback buys, and it does not separate running from awaiting.
      out.push_str(
        "\nThe slowest hooks, timed inside each callback (the wait before a callback starts \
         is excluded, the time it awaits is included):",
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
        // Every hidden row is over the one-second floor. Only the cap dropped it.
        let _ = write!(out, "\n  … and {} more hooks over 1s", self.measured_hidden);
      }
      // These sentences explain the gap between the headline and the rows. Without them, a
      // reader cannot tell whether the gap is unmeasurable callbacks, hooks under the floor,
      // or core work. The comparison uses unclamped percentages: with both clamped at 100%,
      // the shares hide a real difference in either direction. The rows are a sum of spans
      // and the headline is their union. The sign of the difference is meaningful, but its
      // size is not: two listed hooks that overlap each other make the difference smaller,
      // and no omitted hook becomes cheaper. That is why no sentence puts a number on the
      // gap.
      let rest = percent(self.busy_ms) - percent(listed_ms);
      if rest < 0 {
        // Nesting is the usual cause, but not the only one. `rankable` only checks the
        // overlap of a hook with itself, so two different `async` hooks can still run at the
        // same time.
        out.push_str(
          "\nThese rows add up to more than the total. Callbacks can overlap, so the same \
           time counts for more than one row.",
        );
      } else if rest > 0 {
        let mut targets = Vec::new();
        if !self.unmeasurable.is_empty() {
          targets.push("the hooks below");
        }
        if self.measured_hidden > 0 {
          targets.push("the hooks not listed");
        } else if self.below_floor > 0 {
          targets.push("hooks under 1s");
        }
        // The sentence shows no numbers. If it showed the sum next to the headline, a reader
        // would subtract them, and that difference is not the time of the omitted hooks. A
        // gap with nothing omitted cannot happen, because a union is at most the sum of its
        // spans. But a sentence that ends in nothing is worse than no sentence, so the check
        // stays.
        if !targets.is_empty() {
          let _ = write!(out, "\nAdditional hook time came from {}.", targets.join(" and from "));
        }
      }
    }

    if !self.unmeasurable.is_empty() {
      let total = self.unmeasurable.len() + self.unmeasurable_hidden;
      let (verb, its, their) =
        if total == 1 { ("is", "Its", "its") } else { ("are", "Their", "their") };
      let _ = write!(
        out,
        "\n{} hook{} {verb} listed without a time. {its} calls overlapped, so the time inside \
         one call includes work from other calls. To find {their} cost, profile the build \
         with `node --cpu-prof`:",
        total,
        plural(u32::try_from(total).unwrap_or(u32::MAX)),
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
    assert!(message.contains("ran for 8.0s of this 10.0s build (80%)"), "{message}");
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
  fn explains_the_gap_between_the_headline_and_the_rows() {
    // Without this sentence, a reader cannot tell whether the gap between the headline and
    // the rows is unmeasurable callbacks or core work.
    let message = render(
      10_000.0,
      9_600.0,
      vec![row("p", "transform", 5_000.0, 3, 1), row("q", "resolveId", 4_000.0, 9, 12)],
    )
    .unwrap();
    // No row here is under the floor, so the sentence must not name such rows. The sentence
    // has no arithmetic: the rows are a sum of spans and the headline is their union, so
    // their difference is not the cost of the omitted hooks.
    assert!(message.contains("Additional hook time came from the hooks below."), "{message}");
  }

  #[test]
  fn says_how_many_rows_it_dropped() {
    // A bounded view has to say what it bounded.
    let measured = (0..u32::try_from(MAX_MEASURED_ROWS).unwrap() + 3)
      .map(|i| row("p", "transform", f64::from(2_000 + i), 1, 1));
    let overlapped = (0..u32::try_from(MAX_UNMEASURABLE_ROWS).unwrap() + 2)
      .map(|i| row("q", "resolveId", 9_000.0, 1, 2 + i));
    let message = render(100_000.0, 90_000.0, measured.chain(overlapped).collect()).unwrap();

    assert!(message.contains("… and 3 more hooks over 1s"), "{message}");
    assert!(message.contains("… and 2 more"), "{message}");
    assert!(message.contains("5 hooks are listed without a time. Their calls"), "{message}");
    // The remainder went to the unmeasurable rows and to the rows past the cap. "hooks
    // under 1s" would be wrong here.
    assert!(
      message
        .contains("Additional hook time came from the hooks below and from the hooks not listed."),
      "{message}"
    );
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
    assert!(message.contains("ran for 6.0s. The build took 4.0s."), "{message}");
    assert!(message.contains("(100%, 6.0s, 1 call)"), "{message}");
    assert!(!message.contains("150%"), "{message}");
  }

  #[test]
  fn rows_that_add_up_to_more_than_the_total_are_explained_as_overlap() {
    // A Vite build: a `load` hook runs a nested build, and the transforms inside it count
    // for both hooks. The rows add up to 125% of the build, and the headline is at 100%. A
    // sentence such as "the rest is below" would be wrong.
    let message = render(
      7_100.0,
      7_400.0,
      vec![
        row("@rolldown/plugin-babel", "transform", 6_200.0, 1_175, 1),
        row("vite:worker", "load", 1_400.0, 1, 1),
        row("vite:asset", "load", 3_000.0, 402, 8),
      ],
    )
    .unwrap();
    assert!(message.contains("These rows add up to more than the total."), "{message}");
    assert!(!message.contains("Additional hook time"), "{message}");
  }

  #[test]
  fn a_remainder_with_nothing_else_listed_names_hooks_under_the_floor() {
    // 6s of rows against an 8s headline, with no unmeasurable rows and no cap. The gap can
    // only be callbacks under one second, and the message must tell the reader that they
    // exist.
    let message = render(
      10_000.0,
      8_000.0,
      vec![row("p", "transform", 6_000.0, 3, 1), row("tiny", "buildStart", 500.0, 40, 1)],
    )
    .unwrap();
    assert!(message.contains("Additional hook time came from hooks under 1s."), "{message}");
  }

  #[test]
  fn a_remainder_past_the_build_carries_no_number() {
    // 5s of rows on a 4s build with 6s of hook time. A share here would clamp to 100%. A
    // duration would make the reader subtract a sum from a union.
    let message = render(
      4_000.0,
      6_000.0,
      vec![row("late", "closeBundle", 5_000.0, 1, 1), row("q", "load", 2_000.0, 9, 4)],
    )
    .unwrap();
    assert!(message.contains("Additional hook time came from the hooks below."), "{message}");
    assert!(!message.contains("add up to"), "{message}");
  }

  #[test]
  fn a_single_overlapped_hook_is_not_pluralised() {
    let message = render(10_000.0, 8_000.0, vec![row("q", "load", 5_000.0, 9, 4)]).unwrap();
    assert!(message.contains("1 hook is listed without a time. Its calls overlapped"), "{message}");
  }
}
