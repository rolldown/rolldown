use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub enum InvalidOptionType {
  UnsupportedInlineDynamicFormat(String),
  UnsupportedCodeSplittingFormat(String),
  InvalidOutputFile,
  InvalidOutputDirOption,
  NoEntryPoint,
  ManualCodeSplittingWithoutGroups(Vec<String>),
  InvalidContext(String),
  IncludeDependenciesRecursivelyWithConflictPreserveEntrySignatures(String),
  IncludeDependenciesRecursivelyWithImplicitPreserveEntrySignatures,
  InvalidFilenamePattern { pattern: String, pattern_name: String },
  InvalidFilenameSubstitution { name: String, pattern_name: String },
  CodeSplittingDisabledWithMultipleInputs,
  CodeSplittingDisabledWithPreserveModules,
  CodeSplittingDisabledWithManualCodeSplitting,
  HashLengthTooLong { pattern_name: String, received: usize, max: usize },
  HashLengthTooShort { pattern_name: String, received: usize, min: usize, chunk_count: u32 },
  InvalidEmittedFileName(String),
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
          format!("Invalid value \"{format}\" for option \"output.format\" - UMD and IIFE are not supported for code-splitting builds. You may set `output.codeSplitting` to `false` when using dynamic imports.")
        }
        InvalidOptionType::UnsupportedCodeSplittingFormat(format) => {
          format!("Invalid value \"{format}\" for option \"output.format\" - UMD and IIFE are not supported for code-splitting builds. For single entry builds, you can set `output.codeSplitting` to `false` to disable code-splitting.")
        }
        InvalidOptionType::InvalidOutputFile => "Invalid value for option \"output.file\" - When building multiple chunks, the \"output.dir\" option must be used, not \"output.file\". You may set `output.codeSplitting` to `false` when using dynamic imports.".to_string(),
        InvalidOptionType::InvalidOutputDirOption => "Invalid value for option \"output.dir\" - you must set either \"output.file\" for a single-file build or \"output.dir\" when generating multiple chunks.".to_string(),
        InvalidOptionType::NoEntryPoint =>"You must supply `options.input` to rolldown, you should at least provide one entrypoint via `options.input` or `this.emitFile({type: 'chunk', ...})` (https://rollupjs.org/plugin-development/#this-emitfile)".to_string(),
        InvalidOptionType::ManualCodeSplittingWithoutGroups(options) => {
          let options_list = options.join(", ");
          format!("Manual code splitting options ({options_list}) specified without groups. These options have no effect without groups - you should either add groups to use manual code splitting or remove these options.")
        }
        InvalidOptionType::InvalidContext(options) => {
            format!("\"{options}\" is an illegitimate identifier for option \"context\". You may use a legitimate context identifier instead.")
        }
        InvalidOptionType::IncludeDependenciesRecursivelyWithConflictPreserveEntrySignatures(value) => {
          [
            "Invalid option combination detected:",
            "",
            "- codeSplitting.includeDependenciesRecursively = false",
            &format!("- preserveEntrySignatures = \"{value}\""),
            "",
            "To fix:",
            "",
            "- Set `preserveEntrySignatures` either to false or 'allow-extension'",
          ].join("\n")
        }
        InvalidOptionType::IncludeDependenciesRecursivelyWithImplicitPreserveEntrySignatures => {
          [
            "`preserveEntrySignatures: 'allow-extension'` is set implicitly by Rolldown",
            "",
            "- `codeSplitting.includeDependenciesRecursively = false` requires `preserveEntrySignatures` to be either `false` or 'allow-extension'",
            "",
            "To fix:",
            "",
            "- Set `preserveEntrySignatures` either to `false` or 'allow-extension' in your config",
          ].join("\n")
        }
        InvalidOptionType::InvalidFilenamePattern { pattern, pattern_name } => {
          format!(
            "Invalid pattern \"{pattern}\" for \"{pattern_name}\", patterns can be neither absolute nor relative paths. \
             If you want your files to be stored in a subdirectory, write its name without a leading \
             slash like this: subdirectory/pattern."
          )
        }
        InvalidOptionType::InvalidFilenameSubstitution { name, pattern_name } => {
          format!(
            "Invalid substitution \"{name}\" for placeholder \"[name]\" in \"{pattern_name}\" pattern, \
             can be neither absolute nor relative paths."
          )
        }
        InvalidOptionType::CodeSplittingDisabledWithMultipleInputs => {
          "Invalid value \"false\" for option \"output.codeSplitting\" - multiple inputs are not supported when \"output.codeSplitting\" is false.".to_string()
        }
        InvalidOptionType::CodeSplittingDisabledWithPreserveModules => {
          "Invalid value \"false\" for option \"output.codeSplitting\" - this option is not supported for \"output.preserveModules\".".to_string()
        }
        InvalidOptionType::CodeSplittingDisabledWithManualCodeSplitting => {
          "Invalid value \"false\" for option \"output.codeSplitting\" - this option is not supported with manual code splitting groups.".to_string()
        }
        InvalidOptionType::HashLengthTooLong { pattern_name, received, max } => {
          format!("Hashes cannot be longer than {max} characters, received {received}. Check the `{pattern_name}` option.")
        }
        InvalidOptionType::HashLengthTooShort { pattern_name, received, min, chunk_count } => {
          format!("To generate hashes for this number of chunks (currently {chunk_count}), you need a minimum hash size of {min}, received {received}. Check the `{pattern_name}` option.")
        }
        InvalidOptionType::InvalidEmittedFileName(name) => {
          format!("The \"fileName\" or \"name\" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths, received \"{name}\".")
        }
    }
  }
}
