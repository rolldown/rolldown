use derivative::Derivative;

use crate::{FileNameTemplate, OutputFormat};

use super::output_options::SourceMapType;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct NormalizedOutputOptions {
  pub entry_file_names: FileNameTemplate,
  pub chunk_file_names: FileNameTemplate,
  pub dir: String,
  pub format: OutputFormat,
  pub sourcemap: SourceMapType,
  pub banner: Option<String>,
}
