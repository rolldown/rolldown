use super::output_options::SourceMapType;
use crate::Addon;
use crate::{FileNameTemplate, OutputFormat};
use derivative::Derivative;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct NormalizedOutputOptions {
  pub entry_file_names: FileNameTemplate,
  pub chunk_file_names: FileNameTemplate,
  pub dir: String,
  pub format: OutputFormat,
  pub sourcemap: SourceMapType,
  pub banner: Addon,
}
