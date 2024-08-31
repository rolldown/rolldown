use crate::events::BuildEvent;
use crate::{DiagnosticOptions, EventKind};

#[derive(Debug)]
pub enum InvalidOptionTypes {
  UnsupportedCodeSplittingFormat,
}

#[derive(Debug)]
pub struct InvalidOption {
  pub invalid_option_types: InvalidOptionTypes,
  pub option: String,
}

impl BuildEvent for InvalidOption {
  fn kind(&self) -> EventKind {
    EventKind::InvalidOption
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    match &self.invalid_option_types {
      InvalidOptionTypes::UnsupportedCodeSplittingFormat => {
        format!("Invalid value \"{}\" for option \"format\". UMD and IIFE are not supported for code splitting. You may set `output.inlineDynamicImports` to `true` when using dynamic imports.", self.option)
      }
    }
  }
}
