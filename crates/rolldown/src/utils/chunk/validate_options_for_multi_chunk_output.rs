use rolldown_common::{NormalizedBundlerOptions, OutputFormat};
use rolldown_error::{BuildDiagnostic, BuildResult, InvalidOptionType};

pub fn validate_options_for_multi_chunk_output(
  options: &NormalizedBundlerOptions,
) -> BuildResult<()> {
  if matches!(options.format, OutputFormat::Umd | OutputFormat::Iife) {
    Err(BuildDiagnostic::invalid_option(InvalidOptionType::UnsupportedCodeSplittingFormat(
      options.format.to_string(),
    )))?;
  }
  // When inline_dynamic_imports is true, all chunks will be inlined into a single file,
  // so using output.file is valid even if multiple chunks exist in the chunk graph
  if options.file.is_some() && !options.inline_dynamic_imports {
    Err(BuildDiagnostic::invalid_option(InvalidOptionType::InvalidOutputFile))?;
  }
  Ok(())
}
