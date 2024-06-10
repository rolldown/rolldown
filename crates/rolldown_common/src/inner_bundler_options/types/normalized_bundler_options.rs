//! [crate::InputOptions] meant to provide dx-friendly options for the `rolldown` users, but it's not suitable for
//! the `rolldown` internal use.

use std::path::PathBuf;

use rustc_hash::FxHashMap;

use crate::ModuleType;

use super::{
  filename_template::FilenameTemplate, input_item::InputItem, is_external::IsExternal,
  output_format::OutputFormat, output_option::AddonOutputOption, platform::Platform,
  source_map_type::SourceMapType, sourcemap_ignore_list::SourceMapIgnoreList,
  sourcemap_path_transform::SourceMapPathTransform,
};

#[derive(Debug)]
pub struct NormalizedBundlerOptions {
  // --- Input
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub external: Option<IsExternal>,
  pub treeshake: bool,
  pub platform: Platform,
  pub shim_missing_exports: bool,
  /// The key is the extension. Unlike `BundlerOptions`, the extension doesn't start with a dot.
  pub module_types: FxHashMap<String, ModuleType>,
  // --- Output
  pub entry_filenames: FilenameTemplate,
  pub chunk_filenames: FilenameTemplate,
  pub asset_filenames: FilenameTemplate,
  pub dir: String,
  pub format: OutputFormat,
  pub sourcemap: SourceMapType,
  pub banner: Option<AddonOutputOption>,
  pub footer: Option<AddonOutputOption>,
  pub sourcemap_ignore_list: Option<SourceMapIgnoreList>,
  pub sourcemap_path_transform: Option<SourceMapPathTransform>,
}
