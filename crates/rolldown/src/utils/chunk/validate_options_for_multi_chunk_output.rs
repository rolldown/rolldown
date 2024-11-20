use rolldown_common::NormalizedBundlerOptions;
use rolldown_error::{BuildDiagnostic, BuildResult, InvalidOptionType};

pub fn validate_options_for_multi_chunk_output(
  options: &NormalizedBundlerOptions,
) -> BuildResult<()> {
  options.file.as_ref().map_or(Ok(()), |_| {
    Err(BuildDiagnostic::invalid_option(InvalidOptionType::InvalidOutputFile).into())
  })
}
