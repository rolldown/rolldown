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
        raw.get_or_create_for_tsconfig(tsconfig.as_deref(), warnings)
      }
    }
  }
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
