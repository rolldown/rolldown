use crate::options::{
  normalized_input_options::NormalizedInputOptions,
  normalized_output_options::NormalizedOutputOptions,
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
    cwd: raw_input.cwd,
    external: raw_input.external,
    treeshake: raw_input.treeshake,
  };

  // Normalize output options

  let output_options = NormalizedOutputOptions {
    entry_file_names: raw_output.entry_file_names,
    chunk_file_names: raw_output.chunk_file_names,
    dir: raw_output.dir,
    format: raw_output.format,
    sourcemap: raw_output.sourcemap,
  };

  NormalizeOptionsReturn { input_options, output_options, resolve_options }
}
