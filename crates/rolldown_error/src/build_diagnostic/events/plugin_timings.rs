use std::fmt::Write as _;

use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

/// A row must reach this to be worth a line.
const MIN_ROW_MS: f64 = 1_000.0;
const MAX_MEASURED_ROWS: usize = 12;
const MAX_UNMEASURABLE_ROWS: usize = 3;
const TABLE_HEADER: [&str; 6] = ["total", "%build", "calls", "avg", "max", "callback"];

/// The location where the user configured a measured callback.
/// The report uses this value to distinguish plugins, input options, and output options.
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
  /// The longest single span. Execution time only when [`PluginTiming::rankable`].
  pub max_ms: f64,
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
}

impl PluginTimings {
  /// Select and order what is worth showing, or `None` when nothing is.
  ///
  /// Whether the build was plugin-bound at all is a separate question, answered before this
  /// by `BuildTimings::plugins_are_slow` — the clocks for it live on the Rust side.
  pub fn new(build_ms: f64, measurement: PluginTimingsMeasurement) -> Option<Self> {
    let (rankable, overlapped): (Vec<_>, Vec<_>) =
      measurement.rows.into_iter().partition(|row| row.rankable);

    let mut measured: Vec<_> = rankable.into_iter().filter(|row| row.ms >= MIN_ROW_MS).collect();
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
    const DOC_LINK: &str = "https://rolldown.rs/reference/InputOptions.checks#bundlertimings";

    #[expect(clippy::cast_possible_truncation)]
    let percent = |ms: f64| -> i64 { (ms / self.build_ms * 100.0).round() as i64 };
    // `closeBundle` runs after the clock the build was measured with stopped, so a span can
    // outrun the build it is a share of. Clamp rather than print an impossible percentage.
    let share = |ms: f64| -> i64 { percent(ms).min(100) };

    let mut out = if self.busy_ms > self.build_ms {
      // Without this branch, the headline reads "7.4s of this 7.1s build". That looks like
      // a bug.
      format!(
        "JavaScript callbacks ran for {}, longer than the {} build.\nHooks like `closeBundle` \
         run after the build ends.",
        format_duration(self.busy_ms),
        format_duration(self.build_ms)
      )
    } else {
      format!(
        "JavaScript callbacks ran for {} of the {} build ({}%).",
        format_duration(self.busy_ms),
        format_duration(self.build_ms),
        share(self.busy_ms)
      )
    };
    out.push('\n');

    let mut listed_ms = 0.0;
    let mut table = vec![TABLE_HEADER.map(String::from)];
    for row in &self.measured {
      listed_ms += row.ms;
      table.push([
        format_duration(row.ms),
        format!("{}%", share(row.ms)),
        row.calls.to_string(),
        format_duration(row.ms / f64::from(row.calls.max(1))),
        format_duration(row.max_ms),
        callback_label(row),
      ]);
    }
    // A number for these would count the same time more than once and rank them above
    // callbacks doing more work.
    for row in &self.unmeasurable {
      let none = || "-".to_string();
      table.push([none(), none(), row.calls.to_string(), none(), none(), callback_label(row)]);
    }
    write_table(&mut out, &table);
    if self.measured_hidden > 0 {
      let _ = write!(
        out,
        "\n  … and {} more callback{} over 1s",
        self.measured_hidden,
        plural(self.measured_hidden)
      );
    }
    if self.unmeasurable_hidden > 0 {
      let _ = write!(
        out,
        "\n  … and {} more callback{} without a time",
        self.unmeasurable_hidden,
        plural(self.unmeasurable_hidden)
      );
    }

    let mut notes = Vec::new();
    if !self.unmeasurable.is_empty() {
      notes.push(
        "A \"-\" means calls of that hook overlapped, so their times include other work."
          .to_string(),
      );
      notes.push(
        "To see how much CPU time that hook uses, run the build with `node --cpu-prof`."
          .to_string(),
      );
    }
    // The rows are a sum of spans and the headline is their union. Only the sign of the
    // difference means something, so the note gives no number. The comparison uses
    // unclamped percentages, because clamping both at 100% would hide the difference.
    if percent(listed_ms) > percent(self.busy_ms) {
      notes.push(format!(
        "Some callbacks overlapped, so the rows add up to more than {}.",
        format_duration(self.busy_ms)
      ));
    }
    if !notes.is_empty() {
      out.push('\n');
      for note in notes {
        out.push('\n');
        out.push_str(&note);
      }
    }

    let _ = write!(out, "\n\nLearn more at {DOC_LINK}");
    out
  }
}

/// The owners the core invokes directly already name themselves — `input options`,
/// `output options`. Only a plugin name needs saying what it is, which also keeps a plugin
/// called `output options` from reading like one of them.
fn owner_prefix(kind: PluginTimingKind) -> &'static str {
  match kind {
    PluginTimingKind::Plugin => "plugin ",
    PluginTimingKind::OutputOption | PluginTimingKind::InputOption => "",
  }
}

/// `plugin vite:css · transform`, `input options · external`.
fn callback_label(row: &PluginTiming) -> String {
  format!("{}{} · {}", owner_prefix(row.kind), row.owner, row.hook)
}

/// Right-aligns every column but the last. The callback name goes last because its length
/// varies the most.
fn write_table(out: &mut String, rows: &[[String; 6]]) {
  let mut widths = [0; 5];
  for row in rows {
    for (width, cell) in widths.iter_mut().zip(row) {
      *width = (*width).max(cell.chars().count());
    }
  }
  for row in rows {
    out.push_str("\n  ");
    for (cell, width) in row.iter().zip(widths) {
      let _ = write!(out, "{cell:>width$}  ");
    }
    out.push_str(&row[5]);
  }
}

/// `1 callback`, `2 callbacks`.
fn plural(count: usize) -> &'static str {
  if count == 1 { "" } else { "s" }
}

/// `2.5s`, `250ms`, `2.5ms`, `40µs`. The unit is picked after rounding, so 999.6ms reads
/// `1.0s` and not `1000ms`.
fn format_duration(ms: f64) -> String {
  if ms >= 999.5 {
    format!("{:.1}s", ms / 1_000.0)
  } else if ms >= 9.95 {
    format!("{ms:.0}ms")
  } else if ms >= 0.9995 {
    format!("{ms:.1}ms")
  } else {
    format!("{:.0}µs", ms * 1_000.0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn row(owner: &str, hook: &str, ms: f64, calls: u32, max_in_flight: u32) -> PluginTiming {
    row_with_kind(PluginTimingKind::Plugin, owner, hook, ms, calls, max_in_flight)
  }

  fn row_with_kind(
    kind: PluginTimingKind,
    owner: &str,
    hook: &str,
    ms: f64,
    calls: u32,
    max_in_flight: u32,
  ) -> PluginTiming {
    PluginTiming {
      owner: owner.to_string(),
      kind,
      hook: hook.to_string(),
      calls,
      ms,
      max_ms: ms / f64::from(calls),
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
  fn ranks_measured_rows_and_lists_the_ones_it_cannot_measure_last() {
    let message = render(
      10_000.0,
      8_000.0,
      vec![
        PluginTiming { max_ms: 900.0, ..row("slow-plugin", "transform", 6_000.0, 500, 1) },
        row_with_kind(
          PluginTimingKind::OutputOption,
          "output options",
          "codeSplitting groups[].name \"vendor\"",
          2_000.0,
          900,
          1,
        ),
        row("async-plugin", "resolveId", 47_000.0, 3_000, 42),
        row("tiny-plugin", "buildStart", 5.0, 1, 1),
      ],
    )
    .unwrap();

    assert_eq!(
      message,
      [
        "JavaScript callbacks ran for 8.0s of the 10.0s build (80%).",
        "",
        "  total  %build  calls    avg    max  callback",
        "   6.0s     60%    500   12ms  900ms  plugin slow-plugin · transform",
        "   2.0s     20%    900  2.2ms  2.2ms  output options · codeSplitting groups[].name \"vendor\"",
        "      -       -   3000      -      -  plugin async-plugin · resolveId",
        "",
        "A \"-\" means calls of that hook overlapped, so their times include other work.",
        "To see how much CPU time that hook uses, run the build with `node --cpu-prof`.",
        "",
        "Learn more at https://rolldown.rs/reference/InputOptions.checks#bundlertimings",
      ]
      .join("\n")
    );
  }

  #[test]
  fn formats_input_option_owners_for_rows_without_a_time() {
    let message = render(
      10_000.0,
      8_000.0,
      vec![row_with_kind(
        PluginTimingKind::InputOption,
        "input options",
        "external",
        5_000.0,
        9,
        4,
      )],
    )
    .unwrap();

    assert!(message.contains("  input options · external\n"), "{message}");
  }

  #[test]
  fn prefixes_plugin_owners_that_match_option_labels() {
    let message =
      render(10_000.0, 8_000.0, vec![row("output options", "transform", 2_000.0, 1, 1)]).unwrap();

    assert!(message.contains("  plugin output options · transform\n"), "{message}");
  }

  #[test]
  fn says_how_many_rows_it_dropped() {
    let measured = (0..u32::try_from(MAX_MEASURED_ROWS).unwrap() + 3)
      .map(|i| row("p", "transform", f64::from(2_000 + i), 1, 1));
    let overlapped = (0..u32::try_from(MAX_UNMEASURABLE_ROWS).unwrap() + 2)
      .map(|i| row("q", "resolveId", 9_000.0, 1, 2 + i));
    let message = render(100_000.0, 90_000.0, measured.chain(overlapped).collect()).unwrap();

    assert!(message.contains("\n  … and 3 more callbacks over 1s\n"), "{message}");
    assert!(message.contains("\n  … and 2 more callbacks without a time\n"), "{message}");
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
    assert!(
      message.starts_with(
        "JavaScript callbacks ran for 6.0s, longer than the 4.0s build.\n\
         Hooks like `closeBundle` run after the build ends.\n"
      ),
      "{message}"
    );
    assert!(
      message.contains("   6.0s    100%      1  6.0s  6.0s  plugin late · closeBundle"),
      "{message}"
    );
    assert!(!message.contains("150%"), "{message}");
  }

  #[test]
  fn rows_that_add_up_to_more_than_the_total_are_explained_as_overlap() {
    // A Vite build: a `load` hook runs a nested build, and the transforms inside it count
    // for both hooks.
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
    assert!(
      message.contains("\nSome callbacks overlapped, so the rows add up to more than 7.4s.\n"),
      "{message}"
    );
  }

  #[test]
  fn picks_the_duration_unit_after_rounding() {
    for (ms, expected) in [
      (2_500.0, "2.5s"),
      (999.6, "1.0s"),
      (250.0, "250ms"),
      (9.96, "10ms"),
      (2.53, "2.5ms"),
      (0.9996, "1.0ms"),
      (0.04, "40µs"),
    ] {
      assert_eq!(format_duration(ms), expected, "{ms}");
    }
  }
}
