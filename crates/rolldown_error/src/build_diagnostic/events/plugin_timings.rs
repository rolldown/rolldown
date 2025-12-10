use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct PluginTimingInfo {
  pub name: String,
  pub percent: u8,
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
    match self.plugins.len() {
      0 => unreachable!("PluginTimings should have at least one plugin"),
      1 => {
        let p = &self.plugins[0];
        format!(
          "Your build spent significant time in plugins. Here is a breakdown:\n  - {} ({}%)",
          p.name, p.percent
        )
      }
      _ => {
        let plugins_list = self
          .plugins
          .iter()
          .map(|p| format!("  - {} ({}%)", p.name, p.percent))
          .collect::<Vec<_>>()
          .join("\n");

        format!("Your build spent significant time in plugins. Here is a breakdown:\n{plugins_list}")
      }
    }
  }
}
