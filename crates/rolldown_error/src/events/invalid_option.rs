use crate::events::BuildEvent;
use crate::{DiagnosticOptions, EventKind};

#[derive(Debug)]
pub enum InvalidOptionTypes {
  UnsupportedCodeSplittingFormat(String),
  InvalidOutputFile,
}

#[derive(Debug)]
pub struct InvalidOption {
  pub invalid_option_types: InvalidOptionTypes,
}

impl BuildEvent for InvalidOption {
  fn kind(&self) -> EventKind {
    EventKind::InvalidOption
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    match &self.invalid_option_types {
      InvalidOptionTypes::UnsupportedCodeSplittingFormat(format) => {
        format!("Invalid value \"{format}\" for option \"output.format\" - UMD and IIFE are not supported for code splitting. You may set `output.inlineDynamicImports` to `true` when using dynamic imports.")
      }
      InvalidOptionTypes::InvalidOutputFile => "Invalid value for option \"output.file\" - when building multiple chunks, the \"output.dir\" option must be used, not \"output.file\". To inline dynamic imports, set the \"inlineDynamicImports\" option.".to_string(),
    }
  }
}
