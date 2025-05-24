//! [crate::InputOptions] meant to provide dx-friendly options for the `rolldown` users, but it's not suitable for
//! the `rolldown` internal use.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use arcstr::ArcStr;
use oxc::transformer_plugins::InjectGlobalVariablesConfig;
use rolldown_error::EventKindSwitcher;
use rustc_hash::{FxHashMap, FxHashSet};

use super::advanced_chunks_options::AdvancedChunksOptions;
use super::experimental_options::ExperimentalOptions;
use super::legal_comments::LegalComments;
use super::minify_options::MinifyOptions;
use super::output_option::{
  AssetFilenamesOutputOption, ChunkFilenamesOutputOption, PreserveEntrySignatures,
};
use super::sanitize_filename::SanitizeFilename;
use super::treeshake::NormalizedTreeshakeOptions;
use super::watch_option::WatchOption;
use super::{
  is_external::IsExternal, output_exports::OutputExports, output_format::OutputFormat,
  output_option::AddonOutputOption, platform::Platform, source_map_type::SourceMapType,
  sourcemap_ignore_list::SourceMapIgnoreList, sourcemap_path_transform::SourceMapPathTransform,
};
use crate::{
  DeferSyncScanDataOption, EmittedAsset, EsModuleFlag, FilenameTemplate, GlobalsOutputOption,
  HashCharacters, InjectImport, InputItem, InvalidateJsSideCache, LogLevel,
  MakeAbsoluteExternalsRelative, MarkModuleLoaded, ModuleType, OnLog, RollupPreRenderedAsset,
  TransformOptions,
};

#[allow(clippy::struct_excessive_bools)] // Using raw booleans is more clear in this case
#[derive(Debug)]
pub struct NormalizedBundlerOptions {
  // --- Input
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub external: Option<IsExternal>,
  /// corresponding to `false | NormalizedTreeshakeOption`
  pub treeshake: NormalizedTreeshakeOptions,
  pub platform: Platform,
  pub shim_missing_exports: bool,
  /// The key is the extension. Unlike `BundlerOptions`, the extension doesn't start with a dot.
  pub module_types: FxHashMap<Cow<'static, str>, ModuleType>,
  // --- Output
  pub name: Option<String>,
  pub css_entry_filenames: ChunkFilenamesOutputOption,
  pub css_chunk_filenames: ChunkFilenamesOutputOption,
  pub entry_filenames: ChunkFilenamesOutputOption,
  pub chunk_filenames: ChunkFilenamesOutputOption,
  pub asset_filenames: AssetFilenamesOutputOption,
  pub sanitize_filename: SanitizeFilename,
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
  pub minify: MinifyOptions,
  pub extend: bool,
  pub define: Vec<(/* Target to be replaced */ String, /* Replacement */ String)>,
  pub keep_names: bool,
  pub inject: Vec<InjectImport>,
  pub oxc_inject_global_variables_config: InjectGlobalVariablesConfig,
  pub external_live_bindings: bool,
  pub inline_dynamic_imports: bool,
  pub advanced_chunks: Option<AdvancedChunksOptions>,
  pub checks: EventKindSwitcher,
  pub profiler_names: bool,
  pub watch: WatchOption,
  pub legal_comments: LegalComments,
  pub drop_labels: FxHashSet<String>,
  pub polyfill_require: bool,
  pub defer_sync_scan_data: Option<DeferSyncScanDataOption>,
  pub transform_options: TransformOptions,
  pub make_absolute_externals_relative: MakeAbsoluteExternalsRelative,
  pub invalidate_js_side_cache: Option<InvalidateJsSideCache>,
  pub mark_module_loaded: Option<MarkModuleLoaded>,
  pub log_level: Option<LogLevel>,
  pub on_log: Option<OnLog>,
  pub preserve_modules: bool,
  pub virtual_dirname: String,
  pub preserve_modules_root: Option<String>,
  pub preserve_entry_signatures: PreserveEntrySignatures,
}

// This is only used for testing
impl Default for NormalizedBundlerOptions {
  #[allow(clippy::default_trait_access)]
  fn default() -> Self {
    Self {
      input: Default::default(),
      cwd: Default::default(),
      external: Default::default(),
      treeshake: Default::default(),
      platform: Platform::Neutral,
      shim_missing_exports: Default::default(),
      module_types: Default::default(),
      name: Default::default(),
      css_entry_filenames: ChunkFilenamesOutputOption::String(String::new()),
      css_chunk_filenames: ChunkFilenamesOutputOption::String(String::new()),
      entry_filenames: ChunkFilenamesOutputOption::String(String::new()),
      chunk_filenames: ChunkFilenamesOutputOption::String(String::new()),
      asset_filenames: AssetFilenamesOutputOption::String(String::new()),
      sanitize_filename: Default::default(),
      dir: Default::default(),
      out_dir: Default::default(),
      file: Default::default(),
      format: OutputFormat::Esm,
      exports: Default::default(),
      es_module: Default::default(),
      hash_characters: Default::default(),
      globals: GlobalsOutputOption::FxHashMap(FxHashMap::default()),
      sourcemap: Default::default(),
      banner: Default::default(),
      footer: Default::default(),
      intro: Default::default(),
      outro: Default::default(),
      sourcemap_ignore_list: Default::default(),
      sourcemap_path_transform: Default::default(),
      sourcemap_debug_ids: Default::default(),
      experimental: Default::default(),
      minify: MinifyOptions::Disabled,
      extend: Default::default(),
      define: Default::default(),
      keep_names: Default::default(),
      inject: Default::default(),
      oxc_inject_global_variables_config: InjectGlobalVariablesConfig::new(vec![]),
      external_live_bindings: Default::default(),
      inline_dynamic_imports: Default::default(),
      advanced_chunks: Default::default(),
      checks: Default::default(),
      profiler_names: Default::default(),
      watch: Default::default(),
      legal_comments: LegalComments::None,
      drop_labels: Default::default(),
      polyfill_require: Default::default(),
      defer_sync_scan_data: Default::default(),
      transform_options: Default::default(),
      make_absolute_externals_relative: Default::default(),
      invalidate_js_side_cache: Default::default(),
      mark_module_loaded: Default::default(),
      log_level: Default::default(),
      on_log: Default::default(),
      preserve_modules: false,
      virtual_dirname: "_virtual".into(),
      preserve_modules_root: Default::default(),
      preserve_entry_signatures: PreserveEntrySignatures::default(),
    }
  }
}

pub type SharedNormalizedBundlerOptions = Arc<NormalizedBundlerOptions>;

impl NormalizedBundlerOptions {
  pub fn is_sourcemap_enabled(&self) -> bool {
    self.sourcemap.is_some()
  }

  pub fn is_esm_format_with_node_platform(&self) -> bool {
    matches!(self.format, OutputFormat::Esm) && matches!(self.platform, Platform::Node)
  }

  pub fn is_hmr_enabled(&self) -> bool {
    self.experimental.hmr.is_some()
  }

  /// make sure the `polyfill_require` is only valid for `esm` format with `node` platform
  #[inline]
  pub fn polyfill_require_for_esm_format_with_node_platform(&self) -> bool {
    if self.is_esm_format_with_node_platform() {
      return self.polyfill_require;
    }
    true
  }

  pub async fn asset_filename_template(
    &self,
    rollup_pre_rendered_asset: &RollupPreRenderedAsset,
  ) -> anyhow::Result<FilenameTemplate> {
    Ok(FilenameTemplate::new(self.asset_filenames.call(rollup_pre_rendered_asset).await?))
  }

  pub async fn asset_filename_with_file(
    &self,
    file: &EmittedAsset,
  ) -> anyhow::Result<Option<String>> {
    if file.file_name.is_some() {
      return Ok(None);
    }
    // TODO avoid clone
    let rollup_pre_rendered_asset = RollupPreRenderedAsset {
      source: file.source.clone(),
      names: file.name.clone().map_or(vec![], |name| vec![name.into()]),
      original_file_names: file
        .original_file_name
        .clone()
        .map_or(vec![], |original_file_name| vec![original_file_name.into()]),
    };
    let asset_filename = self.asset_filenames.call(&rollup_pre_rendered_asset).await?;
    Ok(Some(asset_filename))
  }

  pub async fn sanitize_file_name_with_file(
    &self,
    file: &EmittedAsset,
  ) -> anyhow::Result<Option<ArcStr>> {
    match file.file_name {
      Some(_) => Ok(None),
      None => Ok(Some(self.sanitize_filename.call(file.name_for_sanitize()).await?)),
    }
  }
}
