use std::fmt::Write as _;

use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

/// Estimated cost of one hook, as a share of the build.
#[derive(Debug)]
pub struct PluginHookTimingInfo {
  pub name: &'static str,
  pub percent: u8,
  pub estimated_ms: u64,
}

/// Estimated cost of everything one plugin — or the output options, for callbacks
/// invoked directly by the Rust core — contributed to the build.
#[derive(Debug)]
pub struct PluginTimingInfo {
  pub name: String,
  pub percent: u8,
  pub estimated_ms: u64,
  /// Per-hook breakdown, most expensive first. Empty when the row covers a single
  /// hook, which would just repeat the row itself.
  pub hooks: Vec<PluginHookTimingInfo>,
}

#[derive(Debug)]
pub struct PluginTimings {
  pub plugins: Vec<PluginTimingInfo>,
}

impl BuildEvent for PluginTimings {
  fn kind(&self) -> EventKind {
    EventKind::PluginTimings
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    const DOC_LINK: &str = "https://rolldown.rs/reference/InputOptions.checks#plugintimings";

    let mut breakdown = String::new();
    for plugin in &self.plugins {
      let _ = write!(
        breakdown,
        "\n  - {} ({}%, ~{})",
        plugin.name,
        plugin.percent,
        format_duration(plugin.estimated_ms)
      );
      for hook in &plugin.hooks {
        let _ = write!(
          breakdown,
          "\n      {} ({}%, ~{})",
          hook.name,
          hook.percent,
          format_duration(hook.estimated_ms)
        );
      }
    }

    format!(
      "Your build spent significant time in plugin hooks. Estimated share of the build:{breakdown}\nSee {DOC_LINK} for more details."
    )
  }
}

#[expect(clippy::cast_precision_loss)]
fn format_duration(ms: u64) -> String {
  if ms >= 1_000 { format!("{:.1}s", ms as f64 / 1_000.0) } else { format!("{ms}ms") }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn renders_owners_with_their_hook_breakdown() {
    let timings = PluginTimings {
      plugins: vec![
        PluginTimingInfo {
          name: "my-plugin".to_string(),
          percent: 61,
          estimated_ms: 6_150,
          hooks: vec![
            PluginHookTimingInfo { name: "transform", percent: 58, estimated_ms: 5_800 },
            PluginHookTimingInfo { name: "resolveId", percent: 3, estimated_ms: 350 },
          ],
        },
        // A single hook is left off: it would just repeat the row above it.
        PluginTimingInfo {
          name: "output options".to_string(),
          percent: 21,
          estimated_ms: 2_100,
          hooks: vec![],
        },
      ],
    };

    let message = timings.message(&DiagnosticOptions::default());
    let breakdown = message.lines().skip(1).take(4).collect::<Vec<_>>();
    assert_eq!(
      breakdown,
      vec![
        "  - my-plugin (61%, ~6.2s)",
        "      transform (58%, ~5.8s)",
        "      resolveId (3%, ~350ms)",
        "  - output options (21%, ~2.1s)",
      ]
    );
  }
}
