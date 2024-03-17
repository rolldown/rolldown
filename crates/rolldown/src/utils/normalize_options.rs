use crate::options::{
  normalized_input_options::NormalizedInputOptions,
  normalized_output_options::NormalizedOutputOptions, output_options::SourceMapType,
};

#[allow(clippy::struct_field_names)]
pub struct NormalizeOptionsReturn {
  pub input_options: NormalizedInputOptions,
  pub output_options: NormalizedOutputOptions,
  pub resolve_options: Option<rolldown_resolver::ResolveOptions>,
}

pub fn normalize_options(
  mut raw_input: crate::InputOptions,
  raw_output: crate::OutputOptions,
) -> NormalizeOptionsReturn {
  let resolve_options = std::mem::take(&mut raw_input.resolve);

  // Normalize input options

  let input_options = NormalizedInputOptions {
    input: raw_input.input,
    cwd: raw_input
      .cwd
      .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir")),
    external: raw_input.external.unwrap_or_default(),
    treeshake: raw_input.treeshake.unwrap_or(true),
  };

  // Normalize output options

  let output_options = NormalizedOutputOptions {
    entry_file_names: raw_output.entry_file_names.unwrap_or_else(|| "[name].js".to_string()).into(),
    chunk_file_names: raw_output
      .chunk_file_names
      .unwrap_or_else(|| "[name]-[hash].js".to_string())
      .into(),
    dir: "dist".to_string(),
    format: raw_output.format.unwrap_or(crate::OutputFormat::Esm),
    sourcemap: raw_output.sourcemap.unwrap_or(SourceMapType::Hidden),
  };

  NormalizeOptionsReturn { input_options, output_options, resolve_options }
}
