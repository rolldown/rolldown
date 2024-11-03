use rolldown_common::NormalizedBundlerOptions;

pub fn validate_options_for_multi_chunk_output(
  options: &NormalizedBundlerOptions,
) -> anyhow::Result<()> {
  // TODO: use `BuildResult`

  if options.file.is_some() {
    return Err(anyhow::format_err!("`file` option is not supported for multi-chunk output"));
  }

  Ok(())
}
