use rolldown_common::ModuleType;
use rolldown_common::side_effects::HookSideEffects;
use rolldown_sourcemap::SourceMap;

/// The sourcemap returned by a `transform` or `renderChunk` plugin's `map` field.
///
/// Mirrors Rollup's behavior where `null` and an omitted `map` field differ:
///
/// - [`Self::Omitted`]: the plugin returned a transform result without setting
///   `map` (JS `undefined`). Treated as a possibly broken sourcemap.
/// - [`Self::Null`]: the plugin returned `map: null` to explicitly signal that
///   no sourcemap is intended for this transformation.
/// - [`Self::Sourcemap`]: the plugin returned an actual sourcemap.
#[derive(Debug, Default, Clone)]
pub enum HookTransformOutputMap {
  #[default]
  Omitted,
  Null,
  Sourcemap(Box<SourceMap>),
}

impl HookTransformOutputMap {
  /// Returns the sourcemap if one was provided.
  pub fn into_sourcemap(self) -> Option<SourceMap> {
    match self {
      Self::Sourcemap(map) => Some(*map),
      Self::Omitted | Self::Null => None,
    }
  }

  pub fn from_if_enabled(enabled: bool, generate: impl FnOnce() -> SourceMap) -> Self {
    if enabled { Self::Sourcemap(Box::new(generate())) } else { Self::Null }
  }
}

impl From<SourceMap> for HookTransformOutputMap {
  fn from(map: SourceMap) -> Self {
    Self::Sourcemap(Box::new(map))
  }
}

#[derive(Debug, Default)]
pub struct HookTransformOutput {
  pub code: Option<String>,
  pub map: HookTransformOutputMap,
  pub side_effects: Option<HookSideEffects>,
  pub module_type: Option<ModuleType>,
}
