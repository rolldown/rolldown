use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub enum InvalidOptionType {
  UnsupportedInlineDynamicFormat(String),
  UnsupportedCodeSplittingFormat(String),
  InvalidOutputFile,
  InvalidOutputDirOption,
  NoEntryPoint,
  AdvancedChunksWithoutGroups(Vec<String>),
  InvalidContext(String),
}

#[derive(Debug)]
pub struct InvalidOption {
  pub invalid_option_type: InvalidOptionType,
}

impl BuildEvent for InvalidOption {
  fn kind(&self) -> EventKind {
    EventKind::InvalidOptionError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    match &self.invalid_option_type {
        InvalidOptionType::UnsupportedInlineDynamicFormat(format) => {
          format!("Invalid value \"{format}\" for option \"output.format\" - UMD and IIFE are not supported for code-splitting builds. You may set `output.inlineDynamicImports` to `true` when using dynamic imports.")
        }
        InvalidOptionType::UnsupportedCodeSplittingFormat(format) => {
          format!("Invalid value \"{format}\" for option \"output.format\" - UMD and IIFE are not supported for code-splitting builds.")
        }
        InvalidOptionType::InvalidOutputFile => "Invalid value for option \"output.file\" - When building multiple chunks, the \"output.dir\" option must be used, not \"output.file\". You may set `output.inlineDynamicImports` to `true` when using dynamic imports.".to_string(),
        InvalidOptionType::InvalidOutputDirOption => "Invalid value for option \"output.dir\" - you must set either \"output.file\" for a single-file build or \"output.dir\" when generating multiple chunks.".to_string(),
        InvalidOptionType::NoEntryPoint =>"You must supply `options.input` to rolldown, you should at least provide one entrypoint via `options.input` or `this.emitFile({type: 'chunk', ...})` (https://rollupjs.org/plugin-development/#this-emitfile)".to_string(),
        InvalidOptionType::AdvancedChunksWithoutGroups(options) => {
          let options_list = options.join(", ");
          format!("Advanced chunks options ({options_list}) specified without groups. These options have no effect without groups - you should either add groups to use advanced chunking or remove these options.")
        }
        InvalidOptionType::InvalidContext(options) => {
            format!("\"{options}\" is an illegitimate identifier for option \"context\". You may use a legitimate context identifier instead.")
        }
    }
  }
}
