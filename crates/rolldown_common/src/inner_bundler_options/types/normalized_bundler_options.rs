//! [crate::InputOptions] meant to provide dx-friendly options for the `rolldown` users, but it's not suitable for
//! the `rolldown` internal use.

use std::path::PathBuf;
use std::sync::Arc;

use oxc::transformer::InjectGlobalVariablesConfig;
use rustc_hash::{FxHashMap, FxHashSet};

use super::advanced_chunks_options::AdvancedChunksOptions;
use super::checks_options::ChecksOptions;
use super::comments::Comments;
use super::experimental_options::ExperimentalOptions;
use super::jsx::Jsx;
use super::output_option::ChunkFilenamesOutputOption;
use super::target::ESTarget;
use super::treeshake::TreeshakeOptions;
use super::watch_option::WatchOption;
use super::{
  filename_template::FilenameTemplate, is_external::IsExternal, output_exports::OutputExports,
  output_format::OutputFormat, output_option::AddonOutputOption, platform::Platform,
  source_map_type::SourceMapType, sourcemap_ignore_list::SourceMapIgnoreList,
  sourcemap_path_transform::SourceMapPathTransform,
};
use crate::{
  EsModuleFlag, GlobalsOutputOption, HashCharacters, InjectImport, InputItem, ModuleType,
};

#[allow(clippy::struct_excessive_bools)] // Using raw booleans is more clear in this case
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
  pub css_entry_filenames: ChunkFilenamesOutputOption,
  pub css_chunk_filenames: ChunkFilenamesOutputOption,
  pub entry_filenames: ChunkFilenamesOutputOption,
  pub chunk_filenames: ChunkFilenamesOutputOption,
  pub asset_filenames: FilenameTemplate,
  // The user specified output directory config
  pub dir: Option<String>,
  // The rolldown resolved output directory from `dir` or `file`.
  pub out_dir: String,
  pub file: Option<String>,
  pub format: OutputFormat,
  pub exports: OutputExports,
  pub es_module: EsModuleFlag,
  pub hash_characters: HashCharacters,
  pub globals: GlobalsOutputOption,
  pub sourcemap: Option<SourceMapType>,
  pub banner: Option<AddonOutputOption>,
  pub footer: Option<AddonOutputOption>,
  pub intro: Option<AddonOutputOption>,
  pub outro: Option<AddonOutputOption>,
  pub sourcemap_ignore_list: Option<SourceMapIgnoreList>,
  pub sourcemap_path_transform: Option<SourceMapPathTransform>,
  pub sourcemap_debug_ids: bool,
  pub experimental: ExperimentalOptions,
  pub minify: bool,
  pub extend: bool,
  pub define: Vec<(/* Target to be replaced */ String, /* Replacement */ String)>,
  pub keep_names: bool,
  pub inject: Vec<InjectImport>,
  pub oxc_inject_global_variables_config: InjectGlobalVariablesConfig,
  pub external_live_bindings: bool,
  pub inline_dynamic_imports: bool,
  pub advanced_chunks: Option<AdvancedChunksOptions>,
  pub checks: ChecksOptions,
  pub profiler_names: bool,
  pub jsx: Jsx,
  pub watch: WatchOption,
  pub comments: Comments,
  pub drop_labels: FxHashSet<String>,
  pub target: ESTarget,
  pub polyfill_require: bool,
}

pub type SharedNormalizedBundlerOptions = Arc<NormalizedBundlerOptions>;

impl NormalizedBundlerOptions {
  pub fn is_sourcemap_enabled(&self) -> bool {
    self.sourcemap.is_some()
  }

  pub fn is_esm_format_with_node_platform(&self) -> bool {
    matches!(self.format, OutputFormat::Esm) && matches!(self.platform, Platform::Node)
  }

  /// make sure the `polyfill_require` is only valid for `esm` format with `node` platform
  #[inline]
  pub fn polyfill_require_for_esm_format_with_node_platform(&self) -> bool {
    if self.is_esm_format_with_node_platform() {
      return self.polyfill_require;
    }
    true
  }
}
