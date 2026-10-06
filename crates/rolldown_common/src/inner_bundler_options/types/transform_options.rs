use std::{
  ops::{Deref, DerefMut},
  path::{Path, PathBuf},
  sync::Arc,
};

use dashmap::Entry;
use oxc::transformer::{ESFeature, EngineTargets, TransformOptions as OxcTransformOptions};
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

/// Every ES feature `oxc_transformer` is able to lower.
///
/// This mirrors oxc's `impl From<EngineTargets> for EnvOptions`: a feature belongs
/// here exactly when that impl turns it into a transform flag. Features oxc merely
/// knows about but cannot lower — the ES2025 RegExp ones — must stay out, or plain
/// JS files would pay for a transform pass that cannot change anything.
///
/// Keep this in sync when upgrading oxc. A missing entry silently stops lowering
/// that syntax in plain JS files, which is how `using` declarations survived an
/// `es2024` target. `tests::env_lowers_anything` below stops compiling when oxc
/// grows a new `EnvOptions` group, so the list gets revisited.
const LOWERABLE_ES_FEATURES: &[ESFeature] = &[
  ESFeature::ES2026ExplicitResourceManagement,
  ESFeature::ES2022ClassStaticBlock,
  ESFeature::ES2022ClassProperties,
  ESFeature::ES2022TopLevelAwait,
  ESFeature::ES2021LogicalAssignmentOperators,
  ESFeature::ES2020ExportNamespaceFrom,
  ESFeature::ES2020NullishCoalescingOperator,
  ESFeature::ES2020OptionalChaining,
  ESFeature::ES2020BigInt,
  ESFeature::ES2020ArbitraryModuleNamespaceNames,
  ESFeature::ES2019OptionalCatchBinding,
  ESFeature::ES2018ObjectRestSpread,
  ESFeature::ES2018AsyncGeneratorFunctions,
  ESFeature::ES2017AsyncToGenerator,
  ESFeature::ES2016ExponentiationOperator,
  ESFeature::ES2015ArrowFunctions,
  ESFeature::ES2015StickyRegex,
  ESFeature::ES2015UnicodeRegex,
  ESFeature::ES2018UnicodePropertyRegex,
  ESFeature::ES2018DotallRegex,
  ESFeature::ES2018NamedCapturingGroupsRegex,
  ESFeature::ES2018LookbehindRegex,
  ESFeature::ES2022MatchIndicesRegex,
  ESFeature::ES2024UnicodeSetsRegex,
];

/// Whether `target` leaves the transformer anything to lower.
///
/// `target` is the single source both `Normal` env options and `Raw` per-file
/// options are derived from, so it answers for both [`TransformOptionsInner`]
/// variants: tsconfig merging never touches it, and oxc builds `EnvOptions` from
/// it with the very same [`EngineTargets::has_feature`] calls made here.
fn target_needs_js_transform(target: &EngineTargets) -> bool {
  LOWERABLE_ES_FEATURES.iter().any(|feature| target.has_feature(*feature))
}

#[derive(Debug, Clone)]
pub struct TransformOptions {
  inner: TransformOptionsInner,
  pub target: EngineTargets,
  pub jsx_preset: JsxPreset,
  /// Whether plain JS files have to go through the transformer. TS and JSX always
  /// do, regardless of the target.
  ///
  /// Derived from `target` by the constructors rather than recomputed on demand:
  /// it is read once per JS module, and answering it walks oxc's static feature
  /// table once per entry in [`LOWERABLE_ES_FEATURES`]. Set it from
  /// [`target_needs_js_transform`] if you ever build this struct by hand.
  pub should_transform_js: bool,
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
    let should_transform_js = target_needs_js_transform(&target);
    Self {
      inner: TransformOptionsInner::Normal(Arc::new(options)),
      target,
      jsx_preset,
      should_transform_js,
    }
  }

  #[inline]
  pub fn new_raw(raw: RawTransformOptions, target: EngineTargets, jsx_preset: JsxPreset) -> Self {
    let should_transform_js = target_needs_js_transform(&target);
    Self { inner: TransformOptionsInner::Raw(raw), target, jsx_preset, should_transform_js }
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
    let target = EngineTargets::default();
    Self {
      inner: TransformOptionsInner::Normal(Arc::new(OxcTransformOptions::default())),
      should_transform_js: target_needs_js_transform(&target),
      target,
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

#[cfg(test)]
mod tests {
  use oxc::transformer::{
    ES2015Options, ES2016Options, ES2017Options, ES2018Options, ES2019Options, ES2020Options,
    ES2021Options, ES2022Options, ES2026Options, EnvOptions,
  };

  use super::*;

  /// [`LOWERABLE_ES_FEATURES`] read from the other side: given the options oxc
  /// derived for a target, is there anything for the transformer to lower?
  ///
  /// `EnvOptions` and its ES-edition groups are destructured exhaustively on
  /// purpose. When an oxc upgrade adds a group — as ES2026 once was — this stops
  /// compiling, which is the prompt to add the feature behind it to
  /// [`LOWERABLE_ES_FEATURES`].
  fn env_lowers_anything(env: EnvOptions) -> bool {
    let EnvOptions {
      // Driven by the TypeScript transform rather than by `target`.
      module: _,
      es2026: ES2026Options { explicit_resource_management },
      es2022: ES2022Options { class_static_block, class_properties, top_level_await },
      es2021: ES2021Options { logical_assignment_operators },
      es2020:
        ES2020Options {
          export_namespace_from,
          nullish_coalescing_operator,
          optional_chaining,
          big_int,
          arbitrary_module_namespace_names,
        },
      es2019: ES2019Options { optional_catch_binding },
      es2018: ES2018Options { object_rest_spread, async_generator_functions },
      es2017: ES2017Options { async_to_generator },
      es2016: ES2016Options { exponentiation_operator },
      es2015: ES2015Options { arrow_function },
      // `RegExpOptions` lives in a private oxc module, so it cannot be named in a
      // pattern and its flags are read field by field below. A brand new RegExp
      // lowering would slip past this canary; the ES-edition groups above are the
      // ones that actually grow.
      regexp,
    } = env;

    explicit_resource_management
      || class_static_block
      || class_properties.is_some()
      || top_level_await
      || logical_assignment_operators
      || export_namespace_from
      || nullish_coalescing_operator
      || optional_chaining
      || big_int
      || arbitrary_module_namespace_names
      || optional_catch_binding
      || object_rest_spread.is_some()
      || async_generator_functions
      || async_to_generator
      || exponentiation_operator
      || arrow_function.is_some()
      || regexp.sticky_flag
      || regexp.unicode_flag
      || regexp.unicode_property_escapes
      || regexp.dot_all_flag
      || regexp.named_capture_groups
      || regexp.look_behind_assertions
      || regexp.match_indices
      || regexp.set_notation
  }

  /// The gate must agree with the options oxc hands the transformer. Saying
  /// `false` while a flag is set skips lowering the target cannot run; saying
  /// `true` while every flag is clear runs a pass that changes nothing.
  #[test]
  fn should_transform_js_matches_oxc_env_options() {
    for target in [
      "es2015",
      "es2016",
      "es2017",
      "es2018",
      "es2019",
      "es2020",
      "es2021",
      "es2022",
      "es2023",
      "es2024",
      "es2025",
      "es2026",
      "esnext",
      // Engine targets do not compare as a single version: Chrome 133 ships
      // unicode-sets RegExp but not `using`, Chrome 134 ships both.
      "chrome133",
      "chrome134",
      "firefox140",
      "firefox141",
      "node22",
      "node24",
    ] {
      let engine_targets = EngineTargets::from_target(target).expect("target should parse");
      let options = TransformOptions::new(
        OxcTransformOptions::default(),
        engine_targets.clone(),
        JsxPreset::default(),
      );

      assert_eq!(
        options.should_transform_js,
        env_lowers_anything(EnvOptions::from(engine_targets)),
        "`should_transform_js` disagrees with oxc's env options for target `{target}`"
      );
    }
  }

  /// The regression behind [`LOWERABLE_ES_FEATURES`]: `es2024` supports
  /// unicode-sets RegExp, so a gate keyed on that feature alone left `using`
  /// declarations unlowered.
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
