//! [crate::InputOptions] meant to provide dx-friendly options for the `rolldown` users, but it's not suitable for
//! the `rolldown` internal use.

use std::path::PathBuf;

use super::{
  external::External, file_name_template::FileNameTemplate, input_item::InputItem,
  output_format::OutputFormat, output_option::AddonOutputOption, platform::Platform,
  source_map_type::SourceMapType,
};

#[derive(Debug)]
pub struct NormalizedBundlerOptions {
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub external: External,
  pub treeshake: bool,
  pub platform: Platform,
  pub entry_file_names: FileNameTemplate,
  pub chunk_file_names: FileNameTemplate,
  pub dir: String,
  pub format: OutputFormat,
  pub sourcemap: SourceMapType,
  pub banner: AddonOutputOption,
  pub footer: AddonOutputOption,
}
