//! The options hash of the persistent scan cache: one `u128` computed from the bundler options.
//!
//! Rules:
//! - Serializable values are hashed. A regular expression is hashed by its pattern and flags.
//! - A function is hashed as a fixed tag. Two different functions give the same hash, but a
//!   function and a string do not.
//! - Maps and sets are hashed in sorted order, so their iteration order does not matter.
//! - Runtime state inside options (caches, resolvers) is not hashed.
//!
//! Structs are destructured without `..`, so a new field fails to compile until someone
//! decides how to hash it.

use std::{borrow::Cow, fmt::Debug, path::Path, sync::Arc};

use arcstr::ArcStr;
use oxc::{
  mangler::MangleOptions,
  minifier::{CompressOptions, ManglePropertiesOptions, MinifierOptions, TreeShakeOptions},
  transformer::EngineTargets,
};
use oxc_str::CompactStr;
use rolldown_error::EventKindSwitcher;
use rolldown_utils::{js_regex::HybridRegex, pattern_filter::StringOrRegex};
use xxhash_rust::xxh3::Xxh3;

use super::types::{
  optimization::{NormalizedInlineConstConfig, NormalizedOptimizationConfig},
  sanitize_filename::SanitizeFilename,
};
use crate::{
  AddonOutputOption, AssetFilenamesOutputOption, AssumeCacheableHooks, AttachDebugInfo,
  BundlerTransformOptions, CacheOptions, ChunkFilenamesOutputOption, ChunkImportMap,
  ChunkModulesOrderBy, ChunkOptimizationOption, ChunkOptimizationOptions, CodeSplittingMode,
  CommentsOptions, CompilerAssumptions, DecoratorOptions, DeferSyncScanDataOption, DevModeOptions,
  EnabledMinifyOptions, EsModuleFlag, ExperimentalOptions, GeneratedCodeOptions,
  GlobalsOutputOption, HashCharacters, InjectImport, InlineCommonChunksOptions, InlineConstMode,
  InnerOptions, InputItem, InvalidateJsSideCache, IsExternal, IsolatedDeclarationsOptions,
  JsxOptions, JsxPreset, LegalComments, LogLevel, MakeAbsoluteExternalsRelative,
  ManglePropertiesPattern, ManglePropertiesPatterns, ManualCodeSplittingOptions, MatchGroup,
  MatchGroupName, MatchGroupTest, MinifyOptions, ModuleSideEffects, ModuleSideEffectsRule,
  ModuleTag, ModuleType, NormalizedBundlerOptions, NormalizedInlineCommonChunksOptions,
  OnInvalidate, OnLog, OutputCharset, OutputExports, OutputFormat, PathsOutputOption, Platform,
  PluginTimingsOption, PluginsOptions, PreserveEntrySignatures, PropertyReadSideEffects,
  PropertyWriteSideEffects, ReactRefreshOptions, ResolveOptions, SourceMapIgnoreList,
  SourceMapPathTransform, SourceMapType, StrictMode, StyledComponentsOptions, TransformOptions,
  TransformOptionsInner, TsConfig, TypeScriptOptions, WatchOption, WatcherFileWatcherOptions,
  bundler_options::Either,
};

/// Hashes the options that decide scan results, for key 1 of the persistent scan cache.
///
/// `resolve` and `tsconfig` are not part of `NormalizedBundlerOptions`, so the caller must keep
/// them before they are moved into the resolver. Plugins are not hashed here.
pub fn options_cache_hash(
  options: &NormalizedBundlerOptions,
  resolve: &ResolveOptions,
  tsconfig: &TsConfig,
) -> u128 {
  let mut h = Xxh3::new();
  options.cache_hash(&mut h);
  resolve.cache_hash(&mut h);
  tsconfig.cache_hash(&mut h);
  h.digest128()
}

trait CacheHash {
  fn cache_hash(&self, h: &mut Xxh3);
}

const FUNCTION_TAG: &str = "<function>";

fn tag(h: &mut Xxh3, name: &str) {
  name.cache_hash(h);
}

/// Each item is hashed on its own and the digests are sorted, so the item order does not matter.
fn hash_unordered<T: CacheHash>(h: &mut Xxh3, items: impl IntoIterator<Item = T>) {
  let mut digests: Vec<u128> = items
    .into_iter()
    .map(|item| {
      let mut item_hasher = Xxh3::new();
      item.cache_hash(&mut item_hasher);
      item_hasher.digest128()
    })
    .collect();
  digests.sort_unstable();
  digests.len().cache_hash(h);
  for digest in digests {
    h.update(&digest.to_le_bytes());
  }
}

/// Hashes the `Debug` output. Use it only for plain data: no functions, no maps or sets, no
/// `HybridRegex`, and no runtime state.
struct ByDebug<'a, T: Debug>(&'a T);

impl<T: Debug> CacheHash for ByDebug<'_, T> {
  fn cache_hash(&self, h: &mut Xxh3) {
    format!("{:?}", self.0).cache_hash(h);
  }
}

// --- Basic types

impl<T: CacheHash + ?Sized> CacheHash for &T {
  fn cache_hash(&self, h: &mut Xxh3) {
    (**self).cache_hash(h);
  }
}

impl<T: CacheHash + ?Sized> CacheHash for Box<T> {
  fn cache_hash(&self, h: &mut Xxh3) {
    (**self).cache_hash(h);
  }
}

impl<T: CacheHash + ?Sized> CacheHash for Arc<T> {
  fn cache_hash(&self, h: &mut Xxh3) {
    (**self).cache_hash(h);
  }
}

impl CacheHash for bool {
  fn cache_hash(&self, h: &mut Xxh3) {
    h.update(&[u8::from(*self)]);
  }
}

macro_rules! impl_cache_hash_for_int {
  ($($ty:ty),*) => {
    $(impl CacheHash for $ty {
      fn cache_hash(&self, h: &mut Xxh3) {
        h.update(&self.to_le_bytes());
      }
    })*
  };
}
impl_cache_hash_for_int!(u8, u16, u32, u64);

impl CacheHash for usize {
  fn cache_hash(&self, h: &mut Xxh3) {
    // Same bytes on 32-bit and 64-bit targets.
    (*self as u64).cache_hash(h);
  }
}

impl CacheHash for f64 {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.to_bits().cache_hash(h);
  }
}

impl CacheHash for str {
  fn cache_hash(&self, h: &mut Xxh3) {
    // The length prefix keeps `"ab" + "c"` apart from `"a" + "bc"`.
    self.len().cache_hash(h);
    h.update(self.as_bytes());
  }
}

impl CacheHash for String {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.as_str().cache_hash(h);
  }
}

impl CacheHash for Cow<'_, str> {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.as_ref().cache_hash(h);
  }
}

impl CacheHash for ArcStr {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.as_str().cache_hash(h);
  }
}

impl CacheHash for CompactStr {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.as_str().cache_hash(h);
  }
}

impl CacheHash for Path {
  fn cache_hash(&self, h: &mut Xxh3) {
    let bytes = self.as_os_str().as_encoded_bytes();
    bytes.len().cache_hash(h);
    h.update(bytes);
  }
}

impl CacheHash for std::path::PathBuf {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.as_path().cache_hash(h);
  }
}

impl<T: CacheHash> CacheHash for Option<T> {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      None => h.update(&[0]),
      Some(value) => {
        h.update(&[1]);
        value.cache_hash(h);
      }
    }
  }
}

impl<T: CacheHash> CacheHash for [T] {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.len().cache_hash(h);
    for item in self {
      item.cache_hash(h);
    }
  }
}

impl<T: CacheHash> CacheHash for Vec<T> {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.as_slice().cache_hash(h);
  }
}

impl<A: CacheHash, B: CacheHash> CacheHash for (A, B) {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.0.cache_hash(h);
    self.1.cache_hash(h);
  }
}

impl<L: CacheHash, R: CacheHash> CacheHash for Either<L, R> {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Either::Left(value) => {
        tag(h, "Left");
        value.cache_hash(h);
      }
      Either::Right(value) => {
        tag(h, "Right");
        value.cache_hash(h);
      }
    }
  }
}

impl CacheHash for HybridRegex {
  fn cache_hash(&self, h: &mut Xxh3) {
    let (source, flags) = self.source_and_flags();
    source.cache_hash(h);
    flags.cache_hash(h);
  }
}

impl CacheHash for StringOrRegex {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      StringOrRegex::String(value) => {
        tag(h, "String");
        value.cache_hash(h);
      }
      StringOrRegex::Regex(regex) => {
        tag(h, "Regex");
        regex.cache_hash(h);
      }
    }
  }
}

impl CacheHash for EventKindSwitcher {
  fn cache_hash(&self, h: &mut Xxh3) {
    self.bits().cache_hash(h);
  }
}

/// Function options. Rolldown cannot compare functions, so only the tag is hashed.
macro_rules! impl_cache_hash_for_function {
  ($($ty:ty),*) => {
    $(impl CacheHash for $ty {
      fn cache_hash(&self, h: &mut Xxh3) {
        tag(h, FUNCTION_TAG);
      }
    })*
  };
}
impl_cache_hash_for_function!(
  DeferSyncScanDataOption,
  InvalidateJsSideCache,
  OnInvalidate,
  OnLog,
  PluginTimingsOption,
  SourceMapPathTransform
);

/// Enums without fields. The match is exhaustive, so a new variant fails to compile.
macro_rules! impl_cache_hash_for_unit_enum {
  ($ty:ty { $($variant:ident),* $(,)? }) => {
    impl CacheHash for $ty {
      fn cache_hash(&self, h: &mut Xxh3) {
        let name = match self {
          $(Self::$variant => stringify!($variant),)*
        };
        tag(h, name);
      }
    }
  };
}
impl_cache_hash_for_unit_enum!(AttachDebugInfo { None, Simple, Full });
impl_cache_hash_for_unit_enum!(ChunkModulesOrderBy { ExecOrder, ModuleId });
impl_cache_hash_for_unit_enum!(EsModuleFlag { Always, Never, IfDefaultProp });
impl_cache_hash_for_unit_enum!(HashCharacters { Base64, Base36, Hex });
impl_cache_hash_for_unit_enum!(InlineConstMode { All, Smart });
impl_cache_hash_for_unit_enum!(JsxPreset { Enable, Disable, Preserve });
impl_cache_hash_for_unit_enum!(LegalComments { None, Inline });
impl_cache_hash_for_unit_enum!(LogLevel { Silent, Warn, Info, Debug });
impl_cache_hash_for_unit_enum!(OutputCharset { Ascii, Utf8 });
impl_cache_hash_for_unit_enum!(OutputExports { Auto, Default, Named, None });
impl_cache_hash_for_unit_enum!(OutputFormat { Esm, Cjs, Iife, Umd });
impl_cache_hash_for_unit_enum!(Platform { Node, Browser, Neutral });
impl_cache_hash_for_unit_enum!(PreserveEntrySignatures {
  AllowExtension,
  Strict,
  ExportsOnly,
  False
});
impl_cache_hash_for_unit_enum!(PropertyReadSideEffects { Always, False });
impl_cache_hash_for_unit_enum!(PropertyWriteSideEffects { Always, False });
impl_cache_hash_for_unit_enum!(SourceMapType { File, Inline, Hidden });
impl_cache_hash_for_unit_enum!(StrictMode { Auto, Always, Never });

// --- Top level

impl CacheHash for NormalizedBundlerOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      input,
      cwd,
      normalized_cwd,
      external,
      treeshake,
      platform,
      shim_missing_exports,
      module_types,
      name,
      entry_filenames,
      chunk_filenames,
      asset_filenames,
      sanitize_filename,
      dir,
      out_dir,
      file,
      charset,
      format,
      exports,
      es_module,
      hash_characters,
      globals,
      paths,
      generated_code,
      sourcemap,
      banner,
      footer,
      post_banner,
      post_footer,
      intro,
      outro,
      sourcemap_ignore_list,
      sourcemap_path_transform,
      sourcemap_debug_ids,
      sourcemap_exclude_sources,
      sourcemap_base_url,
      sourcemap_filenames,
      experimental,
      minify,
      extend,
      define,
      keep_names,
      inject,
      oxc_inject_global_variables_config,
      external_live_bindings,
      code_splitting,
      dynamic_import_in_cjs,
      manual_code_splitting,
      inline_common_chunks,
      checks,
      profiler_names,
      watch,
      legal_comments,
      comments,
      drop_labels,
      polyfill_require,
      // Callbacks into the host. They never change a scan result, so they are skipped, also
      // whether they are set: `build()`, `watch()` and the dev engine set different ones.
      defer_sync_scan_data: _,
      plugin_timings: _,
      transform_options,
      make_absolute_externals_relative,
      invalidate_js_side_cache: _,
      log_level,
      on_log: _,
      preserve_modules,
      virtual_dirname,
      preserve_modules_root,
      preserve_entry_signatures,
      devtools,
      optimization,
      top_level_var,
      minify_internal_exports,
      clean_dir,
      context,
      strict_execution_order,
      strict,
    } = self;

    input.cache_hash(h);
    cwd.cache_hash(h);
    normalized_cwd.cache_hash(h);
    external.cache_hash(h);
    treeshake.as_ref().cache_hash(h);
    platform.cache_hash(h);
    shim_missing_exports.cache_hash(h);
    hash_unordered(h, module_types.iter());
    name.cache_hash(h);
    entry_filenames.cache_hash(h);
    chunk_filenames.cache_hash(h);
    asset_filenames.cache_hash(h);
    sanitize_filename.cache_hash(h);
    dir.cache_hash(h);
    out_dir.cache_hash(h);
    file.cache_hash(h);
    charset.cache_hash(h);
    format.cache_hash(h);
    exports.cache_hash(h);
    es_module.cache_hash(h);
    hash_characters.cache_hash(h);
    globals.cache_hash(h);
    paths.cache_hash(h);
    generated_code.cache_hash(h);
    sourcemap.cache_hash(h);
    banner.cache_hash(h);
    footer.cache_hash(h);
    post_banner.cache_hash(h);
    post_footer.cache_hash(h);
    intro.cache_hash(h);
    outro.cache_hash(h);
    sourcemap_ignore_list.cache_hash(h);
    sourcemap_path_transform.cache_hash(h);
    sourcemap_debug_ids.cache_hash(h);
    sourcemap_exclude_sources.cache_hash(h);
    sourcemap_base_url.cache_hash(h);
    sourcemap_filenames.cache_hash(h);
    experimental.cache_hash(h);
    minify.cache_hash(h);
    extend.cache_hash(h);
    // A list, not a map: the order of `define` entries is kept as the user wrote it.
    define.cache_hash(h);
    keep_names.cache_hash(h);
    inject.cache_hash(h);
    // Safe for `Debug`: a list of import sources and names, built from `inject`.
    ByDebug(oxc_inject_global_variables_config).cache_hash(h);
    external_live_bindings.cache_hash(h);
    code_splitting.cache_hash(h);
    dynamic_import_in_cjs.cache_hash(h);
    manual_code_splitting.cache_hash(h);
    inline_common_chunks.cache_hash(h);
    checks.cache_hash(h);
    profiler_names.cache_hash(h);
    watch.cache_hash(h);
    legal_comments.cache_hash(h);
    comments.cache_hash(h);
    hash_unordered(h, drop_labels.iter());
    polyfill_require.cache_hash(h);
    transform_options.cache_hash(h);
    make_absolute_externals_relative.cache_hash(h);
    log_level.cache_hash(h);
    preserve_modules.cache_hash(h);
    virtual_dirname.cache_hash(h);
    preserve_modules_root.cache_hash(h);
    preserve_entry_signatures.cache_hash(h);
    devtools.cache_hash(h);
    optimization.cache_hash(h);
    top_level_var.cache_hash(h);
    minify_internal_exports.cache_hash(h);
    clean_dir.cache_hash(h);
    context.cache_hash(h);
    strict_execution_order.cache_hash(h);
    strict.cache_hash(h);
  }
}

impl CacheHash for ResolveOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      alias,
      alias_fields,
      condition_names,
      exports_fields,
      extensions,
      extension_alias,
      main_fields,
      main_files,
      modules,
      symlinks,
      yarn_pnp,
    } = self;
    // Lists, not maps: the resolver tries `alias` and `extension_alias` entries in order.
    alias.cache_hash(h);
    alias_fields.cache_hash(h);
    condition_names.cache_hash(h);
    exports_fields.cache_hash(h);
    extensions.cache_hash(h);
    extension_alias.cache_hash(h);
    main_fields.cache_hash(h);
    main_files.cache_hash(h);
    modules.cache_hash(h);
    symlinks.cache_hash(h);
    yarn_pnp.cache_hash(h);
  }
}

impl CacheHash for TsConfig {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      TsConfig::Auto(value) => {
        tag(h, "Auto");
        value.cache_hash(h);
      }
      TsConfig::Manual(path) => {
        tag(h, "Manual");
        path.cache_hash(h);
      }
    }
  }
}

// --- Input options

impl CacheHash for InputItem {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { name, import } = self;
    name.cache_hash(h);
    import.cache_hash(h);
  }
}

impl CacheHash for IsExternal {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      IsExternal::Fn(function) => {
        tag(h, "Fn");
        function.as_ref().map(|_| FUNCTION_TAG).cache_hash(h);
      }
      IsExternal::StringOrRegex(patterns) => {
        tag(h, "StringOrRegex");
        patterns.cache_hash(h);
      }
    }
  }
}

impl CacheHash for InnerOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      module_side_effects,
      annotations,
      manual_pure_functions,
      unknown_global_side_effects,
      commonjs,
      property_read_side_effects,
      property_write_side_effects,
      invalid_import_side_effects,
    } = self;
    module_side_effects.cache_hash(h);
    annotations.cache_hash(h);
    match manual_pure_functions {
      None => tag(h, "None"),
      Some(names) => {
        tag(h, "Some");
        hash_unordered(h, names.iter());
      }
    }
    unknown_global_side_effects.cache_hash(h);
    commonjs.cache_hash(h);
    property_read_side_effects.cache_hash(h);
    property_write_side_effects.cache_hash(h);
    invalid_import_side_effects.cache_hash(h);
  }
}

impl CacheHash for ModuleSideEffects {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      ModuleSideEffects::Rules(rules) => {
        tag(h, "Rules");
        rules.cache_hash(h);
      }
      ModuleSideEffects::IdSet(ids) => {
        tag(h, "IdSet");
        hash_unordered(h, ids.iter());
      }
      ModuleSideEffects::Boolean(value) => {
        tag(h, "Boolean");
        value.cache_hash(h);
      }
      ModuleSideEffects::Function(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for ModuleSideEffectsRule {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { test, external, side_effects } = self;
    test.cache_hash(h);
    external.cache_hash(h);
    side_effects.cache_hash(h);
  }
}

impl CacheHash for ModuleType {
  fn cache_hash(&self, h: &mut Xxh3) {
    let name = match self {
      ModuleType::Js => "Js",
      ModuleType::Jsx => "Jsx",
      ModuleType::Ts => "Ts",
      ModuleType::Tsx => "Tsx",
      ModuleType::Json => "Json",
      ModuleType::Text => "Text",
      ModuleType::Base64 => "Base64",
      ModuleType::Dataurl => "Dataurl",
      ModuleType::Binary => "Binary",
      ModuleType::Empty => "Empty",
      ModuleType::Css => "Css",
      ModuleType::Asset => "Asset",
      ModuleType::Copy => "Copy",
      ModuleType::Custom(custom) => {
        tag(h, "Custom");
        custom.cache_hash(h);
        return;
      }
    };
    tag(h, name);
  }
}

// --- Output options

impl CacheHash for ChunkFilenamesOutputOption {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::String(value) => {
        tag(h, "String");
        value.cache_hash(h);
      }
      Self::Fn(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for AssetFilenamesOutputOption {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::String(value) => {
        tag(h, "String");
        value.cache_hash(h);
      }
      Self::Fn(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for SanitizeFilename {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Boolean(value) => {
        tag(h, "Boolean");
        value.cache_hash(h);
      }
      Self::Fn(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for GlobalsOutputOption {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::FxHashMap(map) => {
        tag(h, "Map");
        hash_unordered(h, map.iter());
      }
      Self::Fn(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for PathsOutputOption {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::FxHashMap(map) => {
        tag(h, "Map");
        hash_unordered(h, map.iter());
      }
      Self::Fn(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for AddonOutputOption {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::String(value) => {
        tag(h, "String");
        value.cache_hash(h);
      }
      Self::Fn(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for SourceMapIgnoreList {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Boolean(value) => {
        tag(h, "Boolean");
        value.cache_hash(h);
      }
      Self::StringOrRegex(pattern) => {
        tag(h, "StringOrRegex");
        pattern.cache_hash(h);
      }
      Self::Fn(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for GeneratedCodeOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { symbols } = self;
    symbols.cache_hash(h);
  }
}

impl CacheHash for CommentsOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { legal, annotation, jsdoc } = self;
    legal.cache_hash(h);
    annotation.cache_hash(h);
    jsdoc.cache_hash(h);
  }
}

impl CacheHash for InjectImport {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      InjectImport::Named { imported, alias, from } => {
        tag(h, "Named");
        imported.cache_hash(h);
        alias.cache_hash(h);
        from.cache_hash(h);
      }
      InjectImport::Namespace { alias, from } => {
        tag(h, "Namespace");
        alias.cache_hash(h);
        from.cache_hash(h);
      }
    }
  }
}

impl CacheHash for MakeAbsoluteExternalsRelative {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Bool(value) => {
        tag(h, "Bool");
        value.cache_hash(h);
      }
      Self::IfRelativeSource => tag(h, "IfRelativeSource"),
    }
  }
}

impl CacheHash for NormalizedOptimizationConfig {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { inline_const, pife_for_module_wrappers } = self;
    inline_const.cache_hash(h);
    pife_for_module_wrappers.cache_hash(h);
  }
}

impl CacheHash for NormalizedInlineConstConfig {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { mode, pass } = self;
    mode.cache_hash(h);
    pass.cache_hash(h);
  }
}

// --- Code splitting

impl CacheHash for CodeSplittingMode {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Bool(value) => {
        tag(h, "Bool");
        value.cache_hash(h);
      }
      Self::Advanced(options) => {
        tag(h, "Advanced");
        options.cache_hash(h);
      }
    }
  }
}

impl CacheHash for ManualCodeSplittingOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      min_share_count,
      min_size,
      max_size,
      min_module_size,
      max_module_size,
      include_dependencies_recursively,
      groups,
      experimental_inline_common_chunks,
      internal_invalidate_module_info_cache,
    } = self;
    min_share_count.cache_hash(h);
    min_size.cache_hash(h);
    max_size.cache_hash(h);
    min_module_size.cache_hash(h);
    max_module_size.cache_hash(h);
    include_dependencies_recursively.cache_hash(h);
    groups.cache_hash(h);
    experimental_inline_common_chunks.cache_hash(h);
    internal_invalidate_module_info_cache.cache_hash(h);
  }
}

impl CacheHash for MatchGroup {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      name,
      test,
      priority,
      min_size,
      max_size,
      min_share_count,
      min_module_size,
      max_module_size,
      entries_aware,
      entries_aware_merge_threshold,
      tags,
      include_dependencies_recursively,
    } = self;
    name.cache_hash(h);
    test.cache_hash(h);
    priority.cache_hash(h);
    min_size.cache_hash(h);
    max_size.cache_hash(h);
    min_share_count.cache_hash(h);
    min_module_size.cache_hash(h);
    max_module_size.cache_hash(h);
    entries_aware.cache_hash(h);
    entries_aware_merge_threshold.cache_hash(h);
    tags.cache_hash(h);
    include_dependencies_recursively.cache_hash(h);
  }
}

impl CacheHash for MatchGroupName {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Static(name) => {
        tag(h, "Static");
        name.cache_hash(h);
      }
      Self::Dynamic(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for MatchGroupTest {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Regex(regex) => {
        tag(h, "Regex");
        regex.cache_hash(h);
      }
      Self::Function(_) => tag(h, FUNCTION_TAG),
    }
  }
}

impl CacheHash for ModuleTag {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Initial => tag(h, "Initial"),
      Self::Custom(name) => {
        tag(h, "Custom");
        name.cache_hash(h);
      }
    }
  }
}

impl CacheHash for InlineCommonChunksOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { max_size, exclude } = self;
    max_size.cache_hash(h);
    exclude.cache_hash(h);
  }
}

impl CacheHash for NormalizedInlineCommonChunksOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { max_size, exclude } = self;
    max_size.cache_hash(h);
    exclude.cache_hash(h);
  }
}

// --- Watch

impl CacheHash for WatchOption {
  fn cache_hash(&self, h: &mut Xxh3) {
    // `on_invalidate` is a callback into the host, like `on_log`.
    let Self { skip_write, build_delay, watcher, include, exclude, on_invalidate: _ } = self;
    skip_write.cache_hash(h);
    build_delay.cache_hash(h);
    watcher.cache_hash(h);
    include.cache_hash(h);
    exclude.cache_hash(h);
  }
}

impl CacheHash for WatcherFileWatcherOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      use_polling,
      poll_interval,
      compare_contents_for_polling,
      use_debounce,
      debounce_delay,
      debounce_tick_rate,
    } = self;
    use_polling.cache_hash(h);
    poll_interval.cache_hash(h);
    compare_contents_for_polling.cache_hash(h);
    use_debounce.cache_hash(h);
    debounce_delay.cache_hash(h);
    debounce_tick_rate.cache_hash(h);
  }
}

// --- Experimental

impl CacheHash for ExperimentalOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      vite_mode,
      resolve_new_url_to_asset,
      incremental_build,
      dev_mode,
      attach_debug_info,
      chunk_import_map,
      chunk_modules_order,
      on_demand_wrapping,
      native_magic_string,
      chunk_optimization,
      lazy_barrel,
      import_attributes,
      cache,
    } = self;
    vite_mode.cache_hash(h);
    resolve_new_url_to_asset.cache_hash(h);
    incremental_build.cache_hash(h);
    dev_mode.cache_hash(h);
    attach_debug_info.cache_hash(h);
    chunk_import_map.cache_hash(h);
    chunk_modules_order.cache_hash(h);
    on_demand_wrapping.cache_hash(h);
    native_magic_string.cache_hash(h);
    chunk_optimization.cache_hash(h);
    lazy_barrel.cache_hash(h);
    import_attributes.cache_hash(h);
    cache.cache_hash(h);
  }
}

impl CacheHash for CacheOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    // `cache_key`, `build_dependencies` and `watch_mode` are separate inputs of key 1. The
    // other fields do not change scan results.
    let Self {
      dir: _,
      max_age: _,
      cache_key: _,
      build_dependencies: _,
      assume_unchanged: _,
      assume_cacheable_hooks,
      watch_mode: _,
    } = self;
    assume_cacheable_hooks.cache_hash(h);
  }
}

impl CacheHash for AssumeCacheableHooks {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { resolve_id, load, transform } = self;
    resolve_id.cache_hash(h);
    load.cache_hash(h);
    transform.cache_hash(h);
  }
}

impl CacheHash for DevModeOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { host, port, implement, skip_common_runtime_injection, lazy, hot_update } = self;
    host.cache_hash(h);
    port.cache_hash(h);
    implement.cache_hash(h);
    skip_common_runtime_injection.cache_hash(h);
    lazy.cache_hash(h);
    hot_update.cache_hash(h);
  }
}

impl CacheHash for ChunkImportMap {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { base_url, file_name } = self;
    base_url.cache_hash(h);
    file_name.cache_hash(h);
  }
}

impl CacheHash for ChunkOptimizationOption {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Bool(value) => {
        tag(h, "Bool");
        value.cache_hash(h);
      }
      Self::Options(options) => {
        tag(h, "Options");
        options.cache_hash(h);
      }
    }
  }
}

impl CacheHash for ChunkOptimizationOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { merge_common_chunks, avoid_redundant_chunk_loads } = self;
    merge_common_chunks.cache_hash(h);
    avoid_redundant_chunk_loads.cache_hash(h);
  }
}

// --- Minify

impl CacheHash for MinifyOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    match self {
      Self::Disabled => tag(h, "Disabled"),
      Self::DeadCodeEliminationOnly(options) => {
        tag(h, "DeadCodeEliminationOnly");
        options.cache_hash(h);
      }
      Self::Enabled(options) => {
        tag(h, "Enabled");
        options.cache_hash(h);
      }
    }
  }
}

impl CacheHash for EnabledMinifyOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { options, remove_whitespace, ascii_only, mangle_properties_patterns } = self;
    options.cache_hash(h);
    remove_whitespace.cache_hash(h);
    ascii_only.cache_hash(h);
    mangle_properties_patterns.cache_hash(h);
  }
}

impl CacheHash for ManglePropertiesPatterns {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { include, exclude } = self;
    include.cache_hash(h);
    exclude.cache_hash(h);
  }
}

impl CacheHash for ManglePropertiesPattern {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { source, flags } = self;
    source.cache_hash(h);
    flags.cache_hash(h);
  }
}

impl CacheHash for MinifierOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { mangle, mangle_properties, compress } = self;
    mangle.cache_hash(h);
    mangle_properties.cache_hash(h);
    compress.cache_hash(h);
  }
}

impl CacheHash for MangleOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { top_level, keep_names, reserved, debug } = self;
    top_level.cache_hash(h);
    // Safe for `Debug`: two booleans.
    ByDebug(keep_names).cache_hash(h);
    hash_unordered(h, reserved.iter());
    debug.cache_hash(h);
  }
}

impl CacheHash for ManglePropertiesOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { include, exclude, reserved, mangle_quoted, debug, cache } = self;
    include.as_str().cache_hash(h);
    match exclude {
      None => tag(h, "None"),
      Some(regex) => {
        tag(h, "Some");
        regex.as_str().cache_hash(h);
      }
    }
    hash_unordered(h, reserved.iter());
    mangle_quoted.cache_hash(h);
    debug.cache_hash(h);
    // User-given name mappings, not runtime state.
    hash_unordered(h, cache.iter());
  }
}

impl CacheHash for CompressOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      target,
      drop_debugger,
      drop_console,
      join_vars,
      sequences,
      unused,
      keep_names,
      treeshake,
      drop_labels,
      max_iterations,
    } = self;
    target.cache_hash(h);
    drop_debugger.cache_hash(h);
    drop_console.cache_hash(h);
    join_vars.cache_hash(h);
    sequences.cache_hash(h);
    // Safe for `Debug`: a field-less enum and a struct of two booleans.
    ByDebug(unused).cache_hash(h);
    ByDebug(keep_names).cache_hash(h);
    treeshake.cache_hash(h);
    hash_unordered(h, drop_labels.iter());
    max_iterations.cache_hash(h);
  }
}

impl CacheHash for TreeShakeOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      annotations,
      manual_pure_functions,
      property_read_side_effects,
      property_write_side_effects,
      unknown_global_side_effects,
      invalid_import_side_effects,
    } = self;
    annotations.cache_hash(h);
    // Built from a set (`treeshake.manualPureFunctions`), so its order has no meaning.
    hash_unordered(h, manual_pure_functions.iter());
    // Safe for `Debug`: a field-less enum.
    ByDebug(property_read_side_effects).cache_hash(h);
    property_write_side_effects.cache_hash(h);
    unknown_global_side_effects.cache_hash(h);
    invalid_import_side_effects.cache_hash(h);
  }
}

impl CacheHash for EngineTargets {
  fn cache_hash(&self, h: &mut Xxh3) {
    // Safe for `Debug`: `Engine` is a field-less enum and `Version` holds three numbers.
    hash_unordered(h, self.iter().map(|(engine, version)| (ByDebug(engine), ByDebug(version))));
  }
}

// --- Transform

impl CacheHash for TransformOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    // `..` covers the private `inner` field, which is reached through `Deref` below.
    let Self { target, jsx_preset, should_transform_js, .. } = self;
    match &**self {
      // Only `base_options` is option data. The other fields are a per-tsconfig cache and
      // the resolver.
      TransformOptionsInner::Raw(raw) => {
        tag(h, "Raw");
        raw.base_options.cache_hash(h);
      }
      TransformOptionsInner::Normal(options) => {
        tag(h, "Normal");
        // Safe for `Debug`: oxc `TransformOptions` holds only strings, paths, booleans,
        // lists and field-less enums.
        ByDebug(options).cache_hash(h);
      }
    }
    target.cache_hash(h);
    jsx_preset.cache_hash(h);
    should_transform_js.cache_hash(h);
  }
}

impl CacheHash for BundlerTransformOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { jsx, target, assumptions, decorator, typescript, plugins, helpers } = self;
    jsx.cache_hash(h);
    target.cache_hash(h);
    assumptions.cache_hash(h);
    decorator.cache_hash(h);
    typescript.cache_hash(h);
    plugins.cache_hash(h);
    // Safe for `Debug`: oxc `HelperLoaderOptions` holds a module name and a field-less enum.
    helpers.as_ref().map(ByDebug).cache_hash(h);
  }
}

impl CacheHash for JsxOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      runtime,
      development,
      throw_if_namespace,
      pure,
      import_source,
      pragma,
      pragma_frag,
      refresh,
    } = self;
    runtime.cache_hash(h);
    development.cache_hash(h);
    throw_if_namespace.cache_hash(h);
    pure.cache_hash(h);
    import_source.cache_hash(h);
    pragma.cache_hash(h);
    pragma_frag.cache_hash(h);
    refresh.cache_hash(h);
  }
}

impl CacheHash for ReactRefreshOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { refresh_reg, refresh_sig, emit_full_signatures } = self;
    refresh_reg.cache_hash(h);
    refresh_sig.cache_hash(h);
    emit_full_signatures.cache_hash(h);
  }
}

impl CacheHash for CompilerAssumptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      ignore_function_length,
      no_document_all,
      object_rest_no_symbols,
      pure_getters,
      set_public_class_fields,
    } = self;
    ignore_function_length.cache_hash(h);
    no_document_all.cache_hash(h);
    object_rest_no_symbols.cache_hash(h);
    pure_getters.cache_hash(h);
    set_public_class_fields.cache_hash(h);
  }
}

impl CacheHash for DecoratorOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { legacy, emit_decorator_metadata, strict_null_checks } = self;
    legacy.cache_hash(h);
    emit_decorator_metadata.cache_hash(h);
    strict_null_checks.cache_hash(h);
  }
}

impl CacheHash for TypeScriptOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      jsx_pragma,
      jsx_pragma_frag,
      only_remove_type_imports,
      allow_namespaces,
      allow_declare_fields,
      remove_class_fields_without_initializer,
      declaration,
      rewrite_import_extensions,
    } = self;
    jsx_pragma.cache_hash(h);
    jsx_pragma_frag.cache_hash(h);
    only_remove_type_imports.cache_hash(h);
    allow_namespaces.cache_hash(h);
    allow_declare_fields.cache_hash(h);
    remove_class_fields_without_initializer.cache_hash(h);
    declaration.cache_hash(h);
    rewrite_import_extensions.cache_hash(h);
  }
}

impl CacheHash for IsolatedDeclarationsOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { strip_internal, sourcemap } = self;
    strip_internal.cache_hash(h);
    sourcemap.cache_hash(h);
  }
}

impl CacheHash for PluginsOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self { styled_components } = self;
    styled_components.cache_hash(h);
  }
}

impl CacheHash for StyledComponentsOptions {
  fn cache_hash(&self, h: &mut Xxh3) {
    let Self {
      display_name,
      file_name,
      ssr,
      transpile_template_literals,
      minify,
      css_prop,
      pure,
      namespace,
      meaningless_file_names,
      top_level_import_paths,
    } = self;
    display_name.cache_hash(h);
    file_name.cache_hash(h);
    ssr.cache_hash(h);
    transpile_template_literals.cache_hash(h);
    minify.cache_hash(h);
    css_prop.cache_hash(h);
    pure.cache_hash(h);
    namespace.cache_hash(h);
    meaningless_file_names.cache_hash(h);
    top_level_import_paths.cache_hash(h);
  }
}

#[cfg(test)]
mod tests {
  use std::{borrow::Cow, sync::Arc};

  use rolldown_utils::{js_regex::HybridRegex, pattern_filter::StringOrRegex};
  use rustc_hash::FxHashMap;

  use super::options_cache_hash;
  use crate::{
    AssumeCacheableHooks, CacheOptions, ChunkFilenamesOutputOption, ExperimentalOptions,
    GlobalsOutputOption, InnerOptions, IsExternal, ModuleType, NormalizedBundlerOptions, Platform,
    ResolveOptions, TsConfig,
  };

  fn hash_of(options: &NormalizedBundlerOptions) -> u128 {
    options_cache_hash(options, &ResolveOptions::default(), &TsConfig::default())
  }

  fn hash_with(edit: impl FnOnce(&mut NormalizedBundlerOptions)) -> u128 {
    let mut options = NormalizedBundlerOptions::default();
    edit(&mut options);
    hash_of(&options)
  }

  fn base_hash() -> u128 {
    hash_of(&NormalizedBundlerOptions::default())
  }

  fn resolve_hash(resolve: &ResolveOptions) -> u128 {
    options_cache_hash(&NormalizedBundlerOptions::default(), resolve, &TsConfig::default())
  }

  fn chunk_filenames_fn() -> ChunkFilenamesOutputOption {
    ChunkFilenamesOutputOption::Fn(Arc::new(|_| Box::pin(async { Ok(String::new()) })))
  }

  #[test]
  fn same_options_give_same_hash() {
    assert_eq!(base_hash(), base_hash());
  }

  #[test]
  fn define_changes_hash() {
    let hash = hash_with(|o| o.define = vec![("A".into(), "1".into())]);
    assert_ne!(hash, base_hash());
    assert_ne!(hash, hash_with(|o| o.define = vec![("A".into(), "2".into())]));
  }

  #[test]
  fn resolve_alias_changes_hash() {
    let base = resolve_hash(&ResolveOptions::default());
    let alias = |to: &str| ResolveOptions {
      alias: Some(vec![("@".into(), vec![Some(to.into())])]),
      ..Default::default()
    };
    assert_ne!(base, resolve_hash(&alias("./src")));
    assert_ne!(resolve_hash(&alias("./src")), resolve_hash(&alias("./lib")));
  }

  #[test]
  fn resolve_condition_names_change_hash() {
    let conditions = |names: &[&str]| ResolveOptions {
      condition_names: Some(names.iter().map(ToString::to_string).collect()),
      ..Default::default()
    };
    assert_ne!(resolve_hash(&conditions(&["import"])), resolve_hash(&conditions(&["require"])));
  }

  #[test]
  fn tsconfig_changes_hash() {
    let options = NormalizedBundlerOptions::default();
    let resolve = ResolveOptions::default();
    let auto = options_cache_hash(&options, &resolve, &TsConfig::Auto(true));
    let off = options_cache_hash(&options, &resolve, &TsConfig::Auto(false));
    let manual_a = options_cache_hash(&options, &resolve, &TsConfig::Manual("a.json".into()));
    let manual_b = options_cache_hash(&options, &resolve, &TsConfig::Manual("b.json".into()));
    assert_ne!(auto, off);
    assert_ne!(auto, manual_a);
    assert_ne!(manual_a, manual_b);
  }

  #[test]
  fn platform_changes_hash() {
    assert_ne!(hash_with(|o| o.platform = Platform::Node), base_hash());
  }

  #[test]
  fn module_types_change_hash() {
    let hash = hash_with(|o| {
      o.module_types.insert(Cow::Borrowed("svg"), ModuleType::Text);
    });
    assert_ne!(hash, base_hash());
  }

  #[test]
  fn map_insertion_order_does_not_change_hash() {
    let pairs = [("js", ModuleType::Js), ("txt", ModuleType::Text), ("svg", ModuleType::Asset)];
    let forward = hash_with(|o| {
      for (ext, module_type) in pairs.clone() {
        o.module_types.insert(Cow::Borrowed(ext), module_type);
      }
    });
    let backward = hash_with(|o| {
      for (ext, module_type) in pairs.clone().into_iter().rev() {
        o.module_types.insert(Cow::Borrowed(ext), module_type);
      }
    });
    assert_eq!(forward, backward);

    let globals = |keys: &[&str]| {
      let mut map = FxHashMap::default();
      for key in keys {
        map.insert((*key).to_string(), key.to_uppercase());
      }
      GlobalsOutputOption::FxHashMap(map)
    };
    assert_eq!(
      hash_with(|o| o.globals = globals(&["a", "b", "c", "d"])),
      hash_with(|o| o.globals = globals(&["d", "c", "b", "a"])),
    );
  }

  #[test]
  fn hash_unordered_ignores_item_order() {
    let digest = |items: &[&str]| {
      let mut h = super::Xxh3::new();
      super::hash_unordered(&mut h, items.iter());
      h.digest128()
    };
    assert_eq!(digest(&["a", "b", "c"]), digest(&["c", "a", "b"]));
    assert_ne!(digest(&["a", "b"]), digest(&["a", "c"]));
  }

  #[test]
  fn treeshake_option_changes_hash() {
    let hash = |annotations: bool| {
      hash_with(|o| {
        o.treeshake =
          InnerOptions { annotations: Some(annotations), ..InnerOptions::default() }.into();
      })
    };
    assert_ne!(hash(true), hash(false));
  }

  #[test]
  fn entry_filenames_change_hash() {
    let hash = |pattern: &str| {
      hash_with(|o| o.entry_filenames = ChunkFilenamesOutputOption::String(pattern.into()))
    };
    assert_ne!(hash("[name].js"), hash("[name].mjs"));
  }

  #[test]
  fn regex_flags_change_hash() {
    let external = |pattern: &str, flags: &str| {
      hash_with(|o| {
        o.external = IsExternal::StringOrRegex(vec![StringOrRegex::Regex(
          HybridRegex::with_flags(pattern, flags).unwrap(),
        )]);
      })
    };
    assert_ne!(external("^lib", ""), external("^lib", "i"));
    // `(?=` needs the ECMAScript engine, which keeps the source and flags itself.
    assert_ne!(external("^lib(?=/)", ""), external("^lib(?=/)", "i"));
    assert_ne!(external("^lib(?=/)", ""), external("^app(?=/)", ""));
  }

  #[test]
  fn functions_hash_as_a_tag() {
    let first = hash_with(|o| o.entry_filenames = chunk_filenames_fn());
    let second = hash_with(|o| o.entry_filenames = chunk_filenames_fn());
    assert_eq!(first, second);

    let string =
      hash_with(|o| o.entry_filenames = ChunkFilenamesOutputOption::String(String::new()));
    assert_ne!(first, string);
  }

  #[test]
  fn host_callbacks_are_skipped() {
    let base = hash_with(|_| {});
    let with_on_log = hash_with(|o| {
      o.on_log = Some(crate::OnLog::new(std::sync::Arc::new(|_, _| Box::pin(async { Ok(()) }))));
    });
    assert_eq!(base, with_on_log);
  }

  #[test]
  fn only_assume_cacheable_hooks_of_cache_options_is_hashed() {
    let with_cache = |cache: CacheOptions| {
      hash_with(|o| {
        o.experimental = ExperimentalOptions { cache: Some(cache), ..Default::default() };
      })
    };
    let base = with_cache(CacheOptions { dir: "a".into(), ..Default::default() });
    let other_dir = with_cache(CacheOptions { dir: "b".into(), ..Default::default() });
    let max_age =
      with_cache(CacheOptions { dir: "a".into(), max_age: Some(1.0), ..Default::default() });
    let hooks = with_cache(CacheOptions {
      dir: "a".into(),
      assume_cacheable_hooks: Some(AssumeCacheableHooks {
        transform: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    });
    assert_eq!(base, other_dir);
    assert_eq!(base, max_age);
    assert_ne!(base, hooks);
  }
}
