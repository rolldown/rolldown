use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct SlowPluginInfo {
  pub name: String,
  pub percent: u8,
}

#[derive(Debug)]
pub struct SlowPlugins {
  pub plugins: Vec<SlowPluginInfo>,
}

impl BuildEvent for SlowPlugins {
  fn kind(&self) -> EventKind {
    EventKind::SlowPlugins
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    match self.plugins.len() {
      0 => unreachable!("SlowPlugins should have at least one plugin"),
      1 => {
        let p = &self.plugins[0];
        format!("This plugin is slowing down your current build: {} ({}%)", p.name, p.percent)
      }
      _ => {
        let plugins_list = self
          .plugins
          .iter()
          .map(|p| format!("  - {} ({}%)", p.name, p.percent))
          .collect::<Vec<_>>()
          .join("\n");

        format!("These plugins are slowing down your current build:\n{plugins_list}")
      }
    }
  }
}
