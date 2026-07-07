use std::fmt::Write as _;

use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub enum InvalidOptionType {
  UnsupportedInlineDynamicFormat(String),
  UnsupportedCodeSplittingFormat(String),
  InvalidOutputFile,
  OutputFileWithoutName(String),
  InvalidOutputDirOption,
  NoEntryPoint,
  ManualCodeSplittingWithoutGroups(Vec<String>),
  InvalidContext(String),
  IncludeDependenciesRecursivelyWithConflictPreserveEntrySignatures(String),
  IncludeDependenciesRecursivelyWithImplicitPreserveEntrySignatures,
  InvalidFilenamePattern {
    pattern: String,
    pattern_name: String,
  },
  InvalidFilenameSubstitution {
    name: String,
    pattern_name: String,
    /// Facade (entry) module of the offending chunk, if any. Used to point users at the source
    /// of an invalid `[name]` substitution.
    facade_module_id: Option<String>,
    /// Module ids contained in the offending chunk, used to help locate where the name came from.
    module_ids: Vec<String>,
  },
  CodeSplittingDisabledWithMultipleInputs,
  CodeSplittingDisabledWithPreserveModules,
  HashLengthTooLong {
    pattern_name: String,
    received: usize,
    max: usize,
  },
  HashLengthTooShort {
    pattern_name: String,
    received: usize,
    min: usize,
    chunk_count: u32,
  },
  InvalidEmittedFileName(String),
  NulByteInFilename {
    pattern_name: String,
  },
}

#[derive(Debug)]
pub struct InvalidOption {
  pub invalid_option_type: InvalidOptionType,
}

impl BuildEvent for InvalidOption {
  fn kind(&self) -> EventKind {
    EventKind::InvalidOptionError
  }

  fn id(&self) -> Option<String> {
    match &self.invalid_option_type {
      InvalidOptionType::InvalidFilenameSubstitution { facade_module_id, module_ids, .. } => {
        facade_module_id.clone().or_else(|| module_ids.first().cloned())
      }
      _ => None,
    }
  }

  fn ids(&self) -> Option<Vec<String>> {
    match &self.invalid_option_type {
      InvalidOptionType::InvalidFilenameSubstitution { module_ids, .. }
        if !module_ids.is_empty() =>
      {
        Some(module_ids.clone())
      }
      _ => None,
    }
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
        InvalidOptionType::OutputFileWithoutName(file) => {
          format!("Invalid value \"{file}\" for option \"output.file\" - it does not contain a file name. Please provide a path that ends with a file name.")
        }
        InvalidOptionType::InvalidOutputDirOption => "Invalid value for option \"output.dir\" - you must set either \"output.file\" for a single-file build or \"output.dir\" when generating multiple chunks.".to_string(),
        InvalidOptionType::NoEntryPoint =>"You must supply `options.input` to rolldown, you should at least provide one entrypoint via `options.input` or `this.emitFile({type: 'chunk', ...})` (https://rolldown.rs/reference/Interface.PluginContext#in-depth-type-chunk)".to_string(),
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
        InvalidOptionType::InvalidFilenameSubstitution { name, pattern_name, facade_module_id, module_ids } => {
          let mut msg = format!(
            "Invalid substitution \"{name}\" for placeholder \"[name]\" in \"{pattern_name}\" pattern, \
             can be neither absolute nor relative paths."
          );
          if let Some(source) =
            facade_module_id.as_deref().or_else(|| module_ids.first().map(String::as_str))
          {
            let _ = write!(msg, "\nThe \"[name]\" was derived from module: {source}");
          }
          if module_ids.len() > 1 {
            const MAX_PREVIEW: usize = 5;
            let preview = module_ids.iter().take(MAX_PREVIEW).cloned().collect::<Vec<_>>().join(", ");
            let suffix = if module_ids.len() > MAX_PREVIEW {
              format!(", ... ({} modules total)", module_ids.len())
            } else {
              String::new()
            };
            let _ = write!(msg, "\nThis chunk contains modules: {preview}{suffix}");
          }
          msg.push_str(
            "\nThis usually happens when an emitted or dynamically imported chunk maps to a module \
             outside the input base (for example inside node_modules), producing a relative \"../\" \
             name. Check the \"name\" passed to this.emitFile({ type: 'chunk', ... }) or your \
             \"output.chunkFileNames\"/\"output.entryFileNames\" option.",
          );
          msg
        }
        InvalidOptionType::CodeSplittingDisabledWithMultipleInputs => {
          "Invalid value \"false\" for option \"output.codeSplitting\" - multiple inputs are not supported when \"output.codeSplitting\" is false.".to_string()
        }
        InvalidOptionType::CodeSplittingDisabledWithPreserveModules => {
          "Invalid value \"false\" for option \"output.codeSplitting\" - this option is not supported for \"output.preserveModules\".".to_string()
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
        InvalidOptionType::NulByteInFilename { pattern_name } => {
          format!("The \"{pattern_name}\" pattern (or the value returned from the function) would result in a filename with invalid null byte(s) (\\0). This is usually caused by using virtual module IDs (which start with \\0) directly in filenames. Use the module ID without the \\0 prefix, or filter out virtual modules from chunk.moduleIds.")
        }
    }
  }
}
