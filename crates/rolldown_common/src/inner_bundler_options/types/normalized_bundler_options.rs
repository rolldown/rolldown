//! [crate::InputOptions] meant to provide dx-friendly options for the `rolldown` users, but it's not suitable for
//! the `rolldown` internal use.

use std::path::PathBuf;

use super::{
  external::External, file_name_template::FilenameTemplate, input_item::InputItem,
  output_format::OutputFormat, output_option::AddonOutputOption, platform::Platform,
  source_map_type::SourceMapType, sourcemap_ignore_list::SourceMapIgnoreList,
  sourcemap_path_transform::SourceMapPathTransform,
};

#[derive(Debug)]
pub struct NormalizedBundlerOptions {
  // --- Input
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub external: Option<External>,
  pub treeshake: bool,
  pub platform: Platform,
  pub shim_missing_exports: bool,
  // --- Output
  pub entry_file_names: FilenameTemplate,
  pub chunk_file_names: FilenameTemplate,
  pub dir: String,
  pub format: OutputFormat,
  pub sourcemap: SourceMapType,
  pub banner: Option<AddonOutputOption>,
  pub footer: Option<AddonOutputOption>,
  pub sourcemap_ignore_list: Option<SourceMapIgnoreList>,
  pub sourcemap_path_transform: Option<SourceMapPathTransform>,
}
