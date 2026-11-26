use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use dashmap::Entry;
use oxc::{
  semantic::Scoping,
  transformer::{
    DecoratorOptions as OxcDecoratorOptions, EngineTargets, EnvOptions,
    TransformOptions as OxcTransformOptions,
  },
};
use oxc_resolver::ResolverGeneric;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::OsFileSystem;
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};

use super::tsconfig_merge::merge_transform_options_with_tsconfig as merge_tsconfig;
use crate::BundlerTransformOptions;

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
  /// Derived from the main resolver and shares its cache, so tsconfig
  /// lookups here and in module resolution stay consistent.
  resolver: Arc<ResolverGeneric<OsFileSystem>>,
  /// Every tsconfig file discovered so far. Survives `clear_cache` so
  /// watchers can still recognize tsconfig files when routing file changes.
  known_tsconfig_paths: FxDashSet<PathBuf>,
}

impl RawTransformOptions {
  pub fn new(
    base_options: BundlerTransformOptions,
    resolver: Arc<ResolverGeneric<OsFileSystem>>,
  ) -> Self {
    Self {
      base_options: Arc::new(base_options),
      cache: FxDashMap::default(),
      resolver,
      known_tsconfig_paths: FxDashSet::default(),
    }
  }

  /// Drop the merged transform options so the next build re-merges them.
  /// The tsconfig contents live in the cache shared with the main resolver.
  pub fn clear_cache(&self) {
    self.cache.clear();
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
  /// Whether plain JS files have to go through the transformer for syntax
  /// lowering other than decorators. TS and JSX always do, regardless of the
  /// target.
  ///
  /// Read once per JS module, so the constructors compute it from oxc's
  /// [`EnvOptions::lowers_syntax`] once.
  pub should_transform_js: bool,
  /// The env and decorator options plain JS files are transformed with. The env
  /// comes from `target`, which both `Normal` and `Raw` options derive theirs
  /// from, and plain JS files never read a tsconfig, so each has a single answer.
  /// See [`Self::js_transforms_decorators`].
  js_env: EnvOptions,
  js_decorator: OxcDecoratorOptions,
}

impl TransformOptions {
  #[inline]
  pub fn new(options: OxcTransformOptions, target: EngineTargets, jsx_preset: JsxPreset) -> Self {
    let js_env = EnvOptions::from(target.clone());
    let js_decorator = options.decorator;
    Self {
      inner: TransformOptionsInner::Normal(Arc::new(options)),
      target,
      jsx_preset,
      should_transform_js: js_env.lowers_syntax(),
      js_env,
      js_decorator,
    }
  }

  #[inline]
  pub fn new_raw(raw: RawTransformOptions, target: EngineTargets, jsx_preset: JsxPreset) -> Self {
    // Plain JS files skip tsconfig merging, so the base options decide for them.
    let js_env = EnvOptions::from(target.clone());
    let js_decorator =
      raw.base_options.decorator.clone().map(OxcDecoratorOptions::from).unwrap_or_default();
    Self {
      inner: TransformOptionsInner::Raw(raw),
      target,
      jsx_preset,
      should_transform_js: js_env.lowers_syntax(),
      js_env,
      js_decorator,
    }
  }

  /// Whether the transformer would lower decorators or `accessor` properties in
  /// the plain JS file `scoping` was built for. No engine ships standard
  /// decorators, so this is decided per file rather than folded into
  /// [`Self::should_transform_js`], which would send every JS module through
  /// the transformer at any concrete target.
  #[inline]
  pub fn js_transforms_decorators(&self, scoping: &Scoping) -> bool {
    self.js_decorator.transforms_decorators(&self.js_env, scoping)
  }

  #[inline]
  pub fn is_jsx_disabled(&self) -> bool {
    matches!(self.jsx_preset, JsxPreset::Disable)
  }

  #[inline]
  pub fn is_jsx_preserve(&self) -> bool {
    matches!(self.jsx_preset, JsxPreset::Preserve)
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
          Some(path) => {
            raw.resolver.find_tsconfig(path).map_err(BuildDiagnostic::tsconfig_error)?
          }
          None => None,
        };
        raw.get_or_create_for_tsconfig(tsconfig.as_deref(), warnings)
      }
    }
  }

  /// Find the tsconfig governing `file_path` so callers can watch it.
  /// Discovery errors are ignored because they surface later in
  /// `options_for_file`.
  pub fn discover_tsconfig_file(&self, file_path: &Path) -> Option<PathBuf> {
    let TransformOptionsInner::Raw(raw) = &self.inner else {
      return None;
    };
    let tsconfig = raw.resolver.find_tsconfig(file_path).ok().flatten()?;
    raw.known_tsconfig_paths.insert(tsconfig.path.clone());
    Some(tsconfig.path.clone())
  }

  /// Whether `path` was discovered as a tsconfig file by an earlier build.
  pub fn is_known_tsconfig(&self, path: &Path) -> bool {
    match &self.inner {
      TransformOptionsInner::Normal(_) => false,
      TransformOptionsInner::Raw(raw) => raw.known_tsconfig_paths.contains(path),
    }
  }

  /// See [RawTransformOptions::clear_cache].
  pub fn clear_transform_tsconfig_cache(&self) {
    if let TransformOptionsInner::Raw(raw) = &self.inner {
      raw.clear_cache();
    }
  }
}

impl Default for TransformOptions {
  fn default() -> Self {
    Self::new(OxcTransformOptions::default(), EngineTargets::default(), JsxPreset::default())
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

#[cfg(test)]
mod tests {
  use super::*;

  /// `es2024` supports unicode-sets RegExp, so a gate keyed on that feature
  /// alone once left `using` declarations unlowered.
  #[test]
  fn target_below_es2026_still_lowers_using() {
    for target in ["es2024", "es2025", "chrome133"] {
      let engine_targets = EngineTargets::from_target(target).expect("target should parse");
      let options =
        TransformOptions::new(OxcTransformOptions::default(), engine_targets, JsxPreset::default());
      assert!(options.should_transform_js, "target `{target}` must lower `using`");
    }
  }

  #[test]
  fn no_target_skips_the_transformer() {
    assert!(!TransformOptions::default().should_transform_js);
  }
}
