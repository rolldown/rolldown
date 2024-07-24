//! [crate::InputOptions] meant to provide dx-friendly options for the `rolldown` users, but it's not suitable for
//! the `rolldown` internal use.

use std::path::PathBuf;

use rustc_hash::FxHashMap;

use super::experimental_options::ExperimentalOptions;
use super::treeshake::TreeshakeOptions;
use super::{
  filename_template::FilenameTemplate, is_external::IsExternal, output_exports::OutputExports,
  output_format::OutputFormat, output_option::AddonOutputOption, platform::Platform,
  source_map_type::SourceMapType, sourcemap_ignore_list::SourceMapIgnoreList,
  sourcemap_path_transform::SourceMapPathTransform,
};
use crate::{InputItem, ModuleType};

#[derive(Debug)]
pub struct NormalizedBundlerOptions {
  // --- Input
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub external: Option<IsExternal>,
  /// corresponding to `false | NormalizedTreeshakeOption`
  pub treeshake: TreeshakeOptions,
  pub platform: Platform,
  pub shim_missing_exports: bool,
  /// The key is the extension. Unlike `BundlerOptions`, the extension doesn't start with a dot.
  pub module_types: FxHashMap<String, ModuleType>,
  // --- Output
  pub name: Option<String>,
  pub entry_filenames: FilenameTemplate,
  pub chunk_filenames: FilenameTemplate,
  pub asset_filenames: FilenameTemplate,
  pub dir: String,
  pub format: OutputFormat,
  pub exports: OutputExports,
  pub globals: FxHashMap<String, String>,
  pub sourcemap: SourceMapType,
  pub banner: Option<AddonOutputOption>,
  pub footer: Option<AddonOutputOption>,
  pub intro: Option<AddonOutputOption>,
  pub outro: Option<AddonOutputOption>,
  pub sourcemap_ignore_list: Option<SourceMapIgnoreList>,
  pub sourcemap_path_transform: Option<SourceMapPathTransform>,
  pub experimental: ExperimentalOptions,
  pub minify: bool,
}
