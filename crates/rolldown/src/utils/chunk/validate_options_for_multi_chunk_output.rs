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
  if options.file.is_some() {
    Err(BuildDiagnostic::invalid_option(InvalidOptionType::InvalidOutputFile))?;
  }
  Ok(())
}
