use crate::events::BuildEvent;
use crate::{DiagnosticOptions, EventKind};

#[derive(Debug)]
pub enum InvalidOptionType {
  UnsupportedCodeSplittingFormat(String),
  InvalidOutputFile,
  InvalidOutputDirOption,
}

#[derive(Debug)]
pub struct InvalidOption {
  pub invalid_option_type: InvalidOptionType,
}

impl BuildEvent for InvalidOption {
  fn kind(&self) -> EventKind {
    EventKind::InvalidOption
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    match &self.invalid_option_type {
      InvalidOptionType::UnsupportedCodeSplittingFormat(format) => {
        format!("Invalid value \"{format}\" for option \"output.format\" - UMD and IIFE are not supported for code splitting. You may set `output.inlineDynamicImports` to `true` when using dynamic imports.")
      }
      InvalidOptionType::InvalidOutputFile => "Invalid value for option \"output.file\" - When building multiple chunks, the \"output.dir\" option must be used, not \"output.file\". You may set `output.inlineDynamicImports` to `true` when using dynamic imports.".to_string(),
      InvalidOptionType::InvalidOutputDirOption => "Invalid value for option \"output.dir\" - you must set either \"output.file\" for a single-file build or \"output.dir\" when generating multiple chunks.".to_string()
    }
  }
}
