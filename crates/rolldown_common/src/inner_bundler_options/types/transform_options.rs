use std::{
  ops::{Deref, DerefMut},
  path::{Path, PathBuf},
  sync::Arc,
};

use dashmap::Entry;
use oxc::transformer::{ESFeature, EngineTargets, TransformOptions as OxcTransformOptions};
use oxc_resolver::{ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::dashmap::FxDashMap;

use super::tsconfig_merge::merge_transform_options_with_tsconfig as merge_tsconfig;
use crate::{BundlerTransformOptions, TsConfig};

#[derive(Debug, Default, Clone)]
pub enum JsxPreset {
  /// Enable JSX transformer
  #[default]
  Enable,
  /// Disable JSX parser - syntax error if JSX is encountered
  Disable,
  /// Parse JSX but preserve it in output
  Preserve,
}

/// Transform options with auto tsconfig discovery and caching
#[derive(Debug, Clone)]
pub struct RawTransformOptions {
  pub base_options: Arc<BundlerTransformOptions>,
  /// Cache key: tsconfig path, or empty PathBuf for files without tsconfig
  pub cache: FxDashMap<PathBuf, Arc<OxcTransformOptions>>,
  resolver: Arc<Resolver>,
}

impl RawTransformOptions {
  pub fn new(base_options: BundlerTransformOptions, tsconfig: TsConfig) -> Self {
    Self {
      base_options: Arc::new(base_options),
      cache: FxDashMap::default(),
      resolver: Arc::new(Resolver::new(ResolveOptions {
        tsconfig: match tsconfig {
          TsConfig::Auto(v) => v.then_some(TsconfigDiscovery::Auto),
          TsConfig::Manual(config_file) => Some(TsconfigDiscovery::Manual(TsconfigOptions {
            config_file,
            references: oxc_resolver::TsconfigReferences::Auto,
          })),
        },
        ..Default::default()
      })),
    }
  }

  pub fn get_or_create_for_tsconfig(
    &self,
    tsconfig: Option<&oxc_resolver::TsConfig>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> BuildResult<Arc<OxcTransformOptions>> {
    let cache_key = tsconfig.map(|t| t.path.clone()).unwrap_or_default();
    match self.cache.entry(cache_key) {
      Entry::Occupied(entry) => Ok(Arc::clone(entry.get())),
      Entry::Vacant(vacant_entry) => {
        let merged_options = Arc::new(merge_transform_options_with_tsconfig(
          self.base_options.as_ref().clone(),
          tsconfig,
          warnings,
        )?);
        vacant_entry.insert(Arc::clone(&merged_options));
        Ok(merged_options)
      }
    }
  }
}

#[derive(Debug, Clone)]
pub enum TransformOptionsInner {
  /// Auto tsconfig discovery - each file uses its nearest tsconfig
  Raw(RawTransformOptions),
  /// Pre-resolved options - all files use the same options
  Normal(Arc<OxcTransformOptions>),
}

#[derive(Debug, Clone)]
pub struct TransformOptions {
  inner: TransformOptionsInner,
  pub target: EngineTargets,
  pub jsx_preset: JsxPreset,
}

impl Deref for TransformOptions {
  type Target = TransformOptionsInner;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl DerefMut for TransformOptions {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

impl TransformOptions {
  #[inline]
  pub fn new(options: OxcTransformOptions, target: EngineTargets, jsx_preset: JsxPreset) -> Self {
    Self { inner: TransformOptionsInner::Normal(Arc::new(options)), target, jsx_preset }
  }

  #[inline]
  pub fn new_raw(raw: RawTransformOptions, target: EngineTargets, jsx_preset: JsxPreset) -> Self {
    Self { inner: TransformOptionsInner::Raw(raw), target, jsx_preset }
  }

  #[inline]
  pub fn is_jsx_disabled(&self) -> bool {
    matches!(self.jsx_preset, JsxPreset::Disable)
  }

  #[inline]
  pub fn is_jsx_preserve(&self) -> bool {
    matches!(self.jsx_preset, JsxPreset::Preserve)
  }

  pub fn should_transform_js(&self) -> bool {
    match &self.inner {
      TransformOptionsInner::Normal(opts) => opts.env.regexp.set_notation,
      TransformOptionsInner::Raw(_) => self.target.has_feature(ESFeature::ES2024UnicodeSetsRegex),
    }
  }

  pub fn options_for_file(
    &self,
    file_path: Option<&Path>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> BuildResult<Arc<OxcTransformOptions>> {
    match &self.inner {
      TransformOptionsInner::Normal(opts) => Ok(Arc::clone(opts)),
      TransformOptionsInner::Raw(raw) => {
        let tsconfig = match file_path {
          Some(path) => raw
            .resolver
            .find_tsconfig(path)
            .map_err(|err| BuildDiagnostic::tsconfig_error(path.display().to_string(), err))?,
          None => None,
        };
        let tsconfig = match (file_path, tsconfig) {
          (Some(path), Some(tsconfig)) => Some(select_tsconfig_for_file(tsconfig, path)),
          (_, tsconfig) => tsconfig,
        };
        raw.get_or_create_for_tsconfig(tsconfig.as_deref(), warnings)
      }
    }
  }
}

fn select_tsconfig_for_file(
  tsconfig: Arc<oxc_resolver::TsConfig>,
  file_path: &Path,
) -> Arc<oxc_resolver::TsConfig> {
  if !is_implicit_solution_tsconfig(&tsconfig) {
    return tsconfig;
  }

  tsconfig
    .references_resolved
    .iter()
    .find(|referenced| is_file_included_in_tsconfig(referenced, file_path))
    .cloned()
    .unwrap_or(tsconfig)
}

fn is_implicit_solution_tsconfig(tsconfig: &oxc_resolver::TsConfig) -> bool {
  if tsconfig.references_resolved.is_empty()
    || tsconfig.files.is_some()
    || tsconfig.include.is_some()
  {
    return false;
  }

  let compiler_options = &tsconfig.compiler_options;
  compiler_options.base_url.is_none()
    && compiler_options.paths.is_none()
    && compiler_options.experimental_decorators.is_none()
    && compiler_options.emit_decorator_metadata.is_none()
    && compiler_options.use_define_for_class_fields.is_none()
    && compiler_options.rewrite_relative_import_extensions.is_none()
    && compiler_options.jsx.is_none()
    && compiler_options.jsx_factory.is_none()
    && compiler_options.jsx_fragment_factory.is_none()
    && compiler_options.jsx_import_source.is_none()
    && compiler_options.verbatim_module_syntax.is_none()
    && compiler_options.preserve_value_imports.is_none()
    && compiler_options.imports_not_used_as_values.is_none()
    && compiler_options.target.is_none()
    && compiler_options.module.is_none()
    && compiler_options.allow_js.is_none()
    && compiler_options.root_dirs.is_none()
}

fn is_file_included_in_tsconfig(tsconfig: &oxc_resolver::TsConfig, file_path: &Path) -> bool {
  if tsconfig
    .files
    .as_ref()
    .is_some_and(|files| files.iter().any(|file| file.as_path() == file_path))
  {
    return true;
  }

  let is_included = match &tsconfig.include {
    Some(include_patterns) => is_glob_matches(tsconfig, file_path, include_patterns),
    None => tsconfig.files.is_none() && is_glob_match(tsconfig, file_path, "**/*"),
  };

  if !is_included {
    return false;
  }

  tsconfig
    .exclude
    .as_ref()
    .is_none_or(|exclude_patterns| !is_glob_matches(tsconfig, file_path, exclude_patterns))
}

fn is_glob_matches(
  tsconfig: &oxc_resolver::TsConfig,
  file_path: &Path,
  patterns: &[std::path::PathBuf],
) -> bool {
  patterns.iter().any(|pattern| {
    let pattern = pattern.to_string_lossy().replace('\\', "/");
    is_glob_match_impl(tsconfig, file_path, &pattern)
  })
}

fn is_glob_match(tsconfig: &oxc_resolver::TsConfig, file_path: &Path, pattern: &str) -> bool {
  is_glob_match_impl(tsconfig, file_path, pattern)
}

fn is_glob_match_impl(tsconfig: &oxc_resolver::TsConfig, file_path: &Path, pattern: &str) -> bool {
  let file_path_str = file_path.to_string_lossy().replace('\\', "/");

  if pattern == file_path_str || pattern == "**/*" {
    return true;
  }

  let mut normalized_pattern = pattern.to_string();
  let after_last_slash =
    normalized_pattern.rsplit('/').next().unwrap_or(normalized_pattern.as_str());
  let needs_implicit_glob = !after_last_slash.contains('.')
    && !after_last_slash.contains('*')
    && !after_last_slash.contains('?');

  if needs_implicit_glob {
    if normalized_pattern.ends_with('/') {
      normalized_pattern.push_str("**/*");
    } else {
      normalized_pattern.push_str("/**/*");
    }
  }

  if normalized_pattern.ends_with('*')
    && !is_file_extension_allowed_in_tsconfig(tsconfig, file_path)
  {
    return false;
  }

  fast_glob::glob_match(&normalized_pattern, &file_path_str)
}

fn is_file_extension_allowed_in_tsconfig(
  tsconfig: &oxc_resolver::TsConfig,
  file_path: &Path,
) -> bool {
  const TS_EXTENSIONS: [&str; 4] = ["ts", "tsx", "mts", "cts"];
  const JS_EXTENSIONS: [&str; 4] = ["js", "jsx", "mjs", "cjs"];

  let allow_js = tsconfig.compiler_options.allow_js.is_some_and(|enabled| enabled);
  file_path
    .extension()
    .and_then(|ext| ext.to_str())
    .is_some_and(|ext| TS_EXTENSIONS.contains(&ext) || (allow_js && JS_EXTENSIONS.contains(&ext)))
}

impl Default for TransformOptions {
  fn default() -> Self {
    Self {
      inner: TransformOptionsInner::Normal(Arc::new(OxcTransformOptions::default())),
      target: EngineTargets::default(),
      jsx_preset: JsxPreset::default(),
    }
  }
}

pub fn merge_transform_options_with_tsconfig(
  transform_options: BundlerTransformOptions,
  tsconfig: Option<&oxc_resolver::TsConfig>,
  warnings: &mut Vec<BuildDiagnostic>,
) -> BuildResult<OxcTransformOptions> {
  let merged_options = if let Some(tsconfig) = tsconfig {
    let (merged, merge_warnings) = merge_tsconfig(transform_options, tsconfig, true);
    warnings.extend(merge_warnings);
    merged
  } else {
    transform_options
  };

  Ok(merged_options.try_into().map_err(|message: String| {
    let hint = message
      .contains("Invalid target")
      .then(|| "Rolldown only supports ES2015 (ES6) and later.".to_owned());
    BuildDiagnostic::bundler_initialize_error(message, hint)
  })?)
}
