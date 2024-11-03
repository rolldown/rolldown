use rolldown_common::NormalizedBundlerOptions;

pub fn validate_options_for_multi_chunk_output(
  options: &NormalizedBundlerOptions,
) -> anyhow::Result<()> {
  // TODO: use `BuildResult`

  if options.file.is_some() {
    return Err(anyhow::format_err!(
      r#"When building multiple chunks, the `dir` option must be used, not `file`. To inline dynamic imports, set the `inlineDynamicImports` option"#
    ));
  }

  Ok(())
}
