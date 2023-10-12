use derivative::Derivative;
use rustc_hash::FxHashMap;

use super::file_name_template::FileNameTemplate;
use crate::OutputOptions;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct NormalizedOutputOptions {
  pub entry_file_names: FileNameTemplate,
  pub chunk_file_names: FileNameTemplate,
  pub manual_chunks: Option<FxHashMap<String, Vec<String>>>,
}

impl NormalizedOutputOptions {
  pub fn from_output_options(opts: OutputOptions) -> Self {
    Self {
      entry_file_names: FileNameTemplate::from(
        opts
          .entry_file_names
          .unwrap_or_else(|| "[name].js".to_string()),
      ),
      chunk_file_names: FileNameTemplate::from(
        opts
          .chunk_file_names
          .unwrap_or_else(|| "[name]-[hash].js".to_string()),
      ),
      manual_chunks: opts.manual_chunks,
    }
  }
}
